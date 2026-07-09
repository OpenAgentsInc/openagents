use serde::{Deserialize, Serialize};

pub const CLOUD_NODE_CONTRACT_VERSION: &str = "openagents.cloud_node.v1";
pub const ARTANIS_BOOTSTRAP_ASSIGNMENT_VERSION: &str = "openagents.artanis_bootstrap_assignment.v1";
pub const CODEX_AUTH_GRANT_VERSION: &str = "openagents.codex_auth_grant.v1";
pub const CODEX_AUTH_RECEIPT_VERSION: &str = "openagents.codex_auth_receipt.v1";
pub const CODEX_WORKROOM_ASSIGNMENT_VERSION: &str = "openagents.codex_workroom_assignment.v1";
pub const CODEX_WORKROOM_EVENT_VERSION: &str = "openagents.codex_workroom_event.v1";
pub const FORGE_ASSIGNMENT_CONTRACT_VERSION: &str = "openagents.forge_assignment.v1";
pub const FORGE_ASSIGNMENT_RECEIPT_VERSION: &str = "openagents.forge_assignment_receipt.v1";
pub const PSIONIC_EXECUTION_RECEIPT_VERSION: &str = "openagents.psionic_execution_receipt.v1";
pub const PSIONIC_WORKER_ATTACHMENT_VERSION: &str = "openagents.psionic_worker_attachment.v1";
pub const PROBE_CLOSEOUT_RECEIPT_VERSION: &str = "openagents.probe_closeout_receipt.v1";
pub const PROBE_WORKER_ATTACHMENT_VERSION: &str = "openagents.probe_worker_attachment.v1";
pub const RESOURCE_USAGE_RECEIPT_VERSION: &str = "openagents.resource_usage_receipt.v1";
pub const TRAINING_RUN_ASSIGNMENT_VERSION: &str = "openagents.training_run_assignment.v1";
pub const WORKROOM_CONTRACT_VERSION: &str = "openagents.workroom.v1";
pub const COMPUTE_QUOTA_ROUTING_VERSION: &str = "openagents.compute_quota_routing.v1";
pub const PLACEMENT_ASSIGNMENT_VERSION: &str = "openagents.codex_placement_assignment.v1";

/// SHC secondary/fallback runner id (CND-041). Google GCE is the primary lane.
pub const SHC_FALLBACK_RUNNER_ID: &str = "oa-shc-katy-01";
/// Stable GCE ephemeral-per-session capacity class id (the commercial-plan C-5
/// class). Owner sessions default to this lane (cloud#88).
pub const GCE_EPHEMERAL_CAPACITY_CLASS_ID: &str = "gce.ephemeral.standard.v1";

// Quota-routing defaults from `openagents.compute_quota_routing.v1`.
/// Hard wall-clock session lifetime: 8h.
pub const DEFAULT_SESSION_TTL_MS: u128 = 8 * 60 * 60 * 1000;
/// Inactivity eviction window: 30m.
pub const DEFAULT_IDLE_TIMEOUT_MS: u128 = 30 * 60 * 1000;
/// Hard remote-lease lifetime: 12h.
pub const DEFAULT_LEASE_TTL_MS: u128 = 12 * 60 * 60 * 1000;
/// Maximum time a paused session may remain paused: 2h.
pub const DEFAULT_PAUSE_TTL_MS: u128 = 2 * 60 * 60 * 1000;
/// Per-owner active-session cap.
pub const DEFAULT_OWNER_ACTIVE_SESSION_CAP: u32 = 4;
/// Per-owner remote-lease cap.
pub const DEFAULT_OWNER_REMOTE_LEASE_CAP: u32 = 2;

// ---------------------------------------------------------------------------
// Placement cost model (CND-042).
//
// These are the single source of truth for the per-lane cost-plus-10% estimate
// used by cost-driven placement. They are derived from the CND-042 receipt
// comparison report:
//   docs/benchmarks/2026-06-14-cnd-042-gce-shc-receipt-comparison.md
//
// To update rates after a real GCP Billing Catalog pull or a real SHC invoice,
// change ONLY the four `*_PER_VM_SEC_NANOUSD` constants below; the cost-plus-10%
// markup and the comparison logic do not need to change.
//
// Units are nano-USD (1e-9 USD) per VM-second to keep integer math precise for
// the small per-second rates without floating point in the decision path. The
// raw (pre-markup) rate is stored; the cost-plus-10% markup is applied in
// `LaneCostModel::cost_plus_10pct_micro_usd_per_vm_sec`.
// ---------------------------------------------------------------------------

/// GCE `e2-small` (us-central1) on-demand list rate, raw (pre-markup).
/// Basis: $0.016751 / VM-hour ÷ 3600 = 4.6531e-6 USD/s = 4653 nano-USD/s.
///
/// This rate is LIST-PRICE-CATALOG-DERIVED (GCP published on-demand list price),
/// NOT a live GCP Cloud Billing export / Billing-Catalog-API metered pull cached
/// per region (see CND-042 report §2.4 unsettled assumption 1, and cloud#92). A
/// resource_usage_receipt that multiplies measured VM-seconds by this rate must
/// therefore record `cost_input_basis = cost_plus_10pct_gcp_catalog`
/// (see [`CostInputBasis`]), never `cost_plus_10pct_gcp` (which is reserved for a
/// live metered Billing export). Pulling a live GCP Billing-export rate and
/// switching the basis to `cost_plus_10pct_gcp` remains the deeper follow-up.
pub const GCE_RAW_PER_VM_SEC_NANOUSD: u64 = 4_653;

/// SHC `oa-shc-katy-01` whole-host amortized rate, raw (pre-markup).
/// Basis: real invoice — $1000.00/year capex, paid upfront (no longer modeled).
/// $1000/yr ÷ 8760h ÷ 3600s = 31.710e-6 USD/s = 31_710 nano-USD/s.
/// (See CND-042 report §2.4 — assumption 2 RESOLVED via the real SHC invoice.)
pub const SHC_RAW_PER_VM_SEC_NANOUSD: u64 = 31_710;

/// Modeled default `standard`-class session length in VM-seconds, used to turn
/// the per-second rates into a per-session cost basis. Rounded from the CND-052
/// mean per-task wall (~260.8 s) to 300 s.
pub const DEFAULT_SESSION_VM_SECONDS: u64 = 300;

/// Margin a cheaper lane must beat the preferred lane by before it can win on
/// cost alone, in basis points (here 10% = 1000 bps). A challenger lane must be
/// at least this much cheaper than GCE to be considered "materially cheaper".
/// This encodes the owner tiebreak: GCE wins ties and near-ties.
pub const PLACEMENT_COST_MATERIAL_MARGIN_BPS: u64 = 1_000;

const MAX_CODEX_AUTH_GRANT_TTL_MS: u128 = 1000 * 60 * 60 * 2;
const SECRET_REF_PREFIXES: &[&str] = &[
    "secret://",
    "vault://",
    "gcp-secret://",
    "cloud-secret://",
    "provider-account://",
    "codex-auth://",
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexAuthGrant {
    pub contract_version: String,
    pub workroom_id: String,
    pub user_ref: String,
    pub organization_ref: Option<String>,
    pub project_ref: Option<String>,
    pub provider_account_ref: String,
    pub grant_ref: String,
    pub provider_secret_ref: String,
    pub requested_mode: CodexRequestedMode,
    pub issued_at_ms: u128,
    pub expires_at_ms: u128,
    pub audit_context: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexRequestedMode {
    Exec,
    McpServer,
    SdkThread,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexAuthReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub workroom_id: String,
    pub grant_ref: String,
    pub provider_account_ref: String,
    pub event_kind: CodexAuthReceiptKind,
    pub decision: CodexAuthDecision,
    pub reason: String,
    pub codex_home_digest: String,
    pub receipt_digest: String,
    pub emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexAuthReceiptKind {
    GrantMaterialized,
    LoginStatusChecked,
    AuthMaterialScrubbed,
    AuthGrantFailed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexAuthDecision {
    Accepted,
    Failed,
}

impl CodexAuthGrant {
    pub fn validate_for_session(&self, now_ms: u128) -> Result<(), String> {
        if self.contract_version != CODEX_AUTH_GRANT_VERSION {
            return Err(format!(
                "unexpected codex auth grant contract version '{}'",
                self.contract_version
            ));
        }
        for (field, value) in [
            ("workroom_id", self.workroom_id.as_str()),
            ("user_ref", self.user_ref.as_str()),
            ("provider_account_ref", self.provider_account_ref.as_str()),
            ("grant_ref", self.grant_ref.as_str()),
            ("provider_secret_ref", self.provider_secret_ref.as_str()),
            ("audit_context", self.audit_context.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
            if contains_secret_material(value) {
                return Err(format!("{field} contains forbidden secret material"));
            }
        }
        for (field, value) in [
            ("organization_ref", self.organization_ref.as_deref()),
            ("project_ref", self.project_ref.as_deref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() || contains_secret_material(value) {
                    return Err(format!("{field} is invalid"));
                }
            }
        }
        if !SECRET_REF_PREFIXES
            .iter()
            .any(|prefix| self.provider_secret_ref.starts_with(prefix))
        {
            return Err("provider_secret_ref must be a server-side secret reference".to_string());
        }
        if self.expires_at_ms <= now_ms {
            return Err("codex auth grant is expired".to_string());
        }
        if self.expires_at_ms <= self.issued_at_ms {
            return Err("codex auth grant expires before it is issued".to_string());
        }
        if self.expires_at_ms - self.issued_at_ms > MAX_CODEX_AUTH_GRANT_TTL_MS {
            return Err("codex auth grant ttl exceeds two hours".to_string());
        }
        Ok(())
    }
}

impl CodexAuthReceipt {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.schema_version != CODEX_AUTH_RECEIPT_VERSION {
            return Err(format!(
                "unexpected codex auth receipt schema '{}'",
                self.schema_version
            ));
        }
        for (field, value) in [
            ("receipt_id", self.receipt_id.as_str()),
            ("workroom_id", self.workroom_id.as_str()),
            ("grant_ref", self.grant_ref.as_str()),
            ("provider_account_ref", self.provider_account_ref.as_str()),
            ("reason", self.reason.as_str()),
            ("codex_home_digest", self.codex_home_digest.as_str()),
            ("receipt_digest", self.receipt_digest.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
            if contains_secret_material(value) {
                return Err(format!("{field} contains forbidden secret material"));
            }
        }
        if !self.codex_home_digest.starts_with("sha256:")
            || !self.receipt_digest.starts_with("sha256:")
        {
            return Err("codex auth receipt digests must start with sha256:".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexWorkroomAssignment {
    pub contract_version: String,
    pub assignment_id: String,
    pub workroom_id: String,
    pub target_node_id: String,
    pub user_ref: String,
    pub organization_ref: Option<String>,
    pub project_ref: Option<String>,
    pub provider_account_ref: String,
    pub auth_grant_ref: String,
    pub repo_ref: String,
    pub prompt: String,
    pub required_artifacts: Vec<String>,
    pub sandbox: CodexSandboxMode,
    pub timeout_ms: Option<u128>,
    pub wallet_authority: bool,
    pub created_at_ms: u128,
    pub audit_context: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexSandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexWorkroomEvent {
    pub schema_version: String,
    pub event_id: String,
    pub assignment_id: String,
    pub workroom_id: String,
    pub sequence: u64,
    pub event_kind: CodexWorkroomEventKind,
    pub decision: CodexWorkroomDecision,
    pub message: String,
    pub artifact_ref: Option<String>,
    pub receipt_ref: Option<String>,
    pub event_digest: String,
    pub emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexWorkroomEventKind {
    Queued,
    Started,
    Log,
    Redacted,
    Artifact,
    Receipt,
    Completed,
    Failed,
    Timeout,
    Cancelled,
    Cleanup,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexWorkroomDecision {
    Accepted,
    Failed,
}

impl CodexWorkroomAssignment {
    pub fn validate_contract(&self, now_ms: u128) -> Result<(), String> {
        if self.contract_version != CODEX_WORKROOM_ASSIGNMENT_VERSION {
            return Err(format!(
                "unexpected codex workroom assignment contract version '{}'",
                self.contract_version
            ));
        }
        for (field, value) in [
            ("assignment_id", self.assignment_id.as_str()),
            ("workroom_id", self.workroom_id.as_str()),
            ("target_node_id", self.target_node_id.as_str()),
            ("user_ref", self.user_ref.as_str()),
            ("provider_account_ref", self.provider_account_ref.as_str()),
            ("auth_grant_ref", self.auth_grant_ref.as_str()),
            ("repo_ref", self.repo_ref.as_str()),
            ("prompt", self.prompt.as_str()),
            ("audit_context", self.audit_context.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
            if contains_secret_material(value) {
                return Err(format!("{field} contains forbidden secret material"));
            }
        }
        for (field, value) in [
            ("organization_ref", self.organization_ref.as_deref()),
            ("project_ref", self.project_ref.as_deref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() || contains_secret_material(value) {
                    return Err(format!("{field} is invalid"));
                }
            }
        }
        if self.wallet_authority {
            return Err("codex workrooms must not receive wallet authority".to_string());
        }
        if self.required_artifacts.is_empty() {
            return Err("codex workroom assignment requires at least one artifact".to_string());
        }
        for artifact in &self.required_artifacts {
            if artifact.trim().is_empty()
                || artifact.contains('/')
                || artifact.contains('\\')
                || artifact.contains("..")
                || contains_secret_material(artifact)
            {
                return Err("codex workroom artifact names must be bounded".to_string());
            }
        }
        if let Some(timeout_ms) = self.timeout_ms {
            if timeout_ms == 0 || timeout_ms > 1000 * 60 * 60 {
                return Err("codex workroom timeout must be between 1ms and 1h".to_string());
            }
        }
        if self.created_at_ms > now_ms + 1000 * 60 * 5 {
            return Err("codex workroom assignment is from the future".to_string());
        }
        Ok(())
    }
}

impl CodexWorkroomEvent {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.schema_version != CODEX_WORKROOM_EVENT_VERSION {
            return Err(format!(
                "unexpected codex workroom event schema '{}'",
                self.schema_version
            ));
        }
        for (field, value) in [
            ("event_id", self.event_id.as_str()),
            ("assignment_id", self.assignment_id.as_str()),
            ("workroom_id", self.workroom_id.as_str()),
            ("message", self.message.as_str()),
            ("event_digest", self.event_digest.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
            if contains_secret_material(value) {
                return Err(format!("{field} contains forbidden secret material"));
            }
        }
        for (field, value) in [
            ("artifact_ref", self.artifact_ref.as_deref()),
            ("receipt_ref", self.receipt_ref.as_deref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() || contains_secret_material(value) {
                    return Err(format!("{field} is invalid"));
                }
                if !value.starts_with("sha256:") {
                    return Err(format!("{field} must be a sha256 digest reference"));
                }
            }
        }
        if !self.event_digest.starts_with("sha256:") {
            return Err("codex workroom event digest must start with sha256:".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceUsageReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub run_ref: String,
    pub workroom_id: String,
    pub node_ref: String,
    pub provider_lane: ProviderLane,
    pub host: ResourceHostSnapshot,
    pub run: RunResourceUsage,
    pub model_usage: Vec<ModelUsageRecord>,
    /// Infra compute metering + billing input (`compute_usage` sub-record from
    /// `openagents.compute_quota_routing.v1`). Present for managed cloud-lane
    /// sessions (GCE) where VM-seconds and a cost-plus-10% billing input are
    /// captured; `None` for control-plane/local paths that meter no VM-seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_usage: Option<ComputeUsage>,
    pub receipt_digest: String,
    pub emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLane {
    Local,
    Gcp,
    Shc,
    Provider,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceHostSnapshot {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub logical_cpu_count: u64,
    pub physical_cpu_count: Option<u64>,
    pub memory_total_bytes: Option<u64>,
    pub memory_available_bytes: Option<u64>,
    pub disk_total_bytes: Option<u64>,
    pub disk_available_bytes: Option<u64>,
    pub accelerator_inventory: Vec<String>,
    pub virtualization: VirtualizationFacts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VirtualizationFacts {
    pub kvm_present: bool,
    pub firecracker_candidate: bool,
    pub container_runtime: Option<String>,
    pub cgroup_mode: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunResourceUsage {
    pub sandbox: String,
    pub image_or_profile_digest: String,
    pub workspace_digest: String,
    pub wall_time_ms: Option<u128>,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub workspace_bytes: Option<u64>,
    pub artifact_bytes: Option<u64>,
    pub log_bytes: Option<u64>,
}

/// How the cost figure in a [`ComputeUsage`] record was formed
/// (`openagents.compute_quota_routing.v1`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostInputBasis {
    /// `cost_input_microusd` = floor(VM-seconds × cost-plus-10% over a LIVE GCP
    /// Cloud Billing export / Billing-Catalog-API metered rate). Reserved for the
    /// deeper follow-up once a live billing export exists; not yet emitted.
    CostPlus10pctGcp,
    /// `cost_input_microusd` = floor(VM-seconds × cost-plus-10% over the GCP
    /// published list-price catalog rate ([`GCE_RAW_PER_VM_SEC_NANOUSD`])). The
    /// VM-seconds are genuinely measured from the lease wall-time; only the rate
    /// is catalog/list-price-derived rather than a live metered Billing export
    /// (cloud#92, CND-042 report §2.4).
    CostPlus10pctGcpCatalog,
    /// No billing input available; `cost_input_microusd` must be `null`.
    Unavailable,
}

impl CostInputBasis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CostPlus10pctGcp => "cost_plus_10pct_gcp",
            Self::CostPlus10pctGcpCatalog => "cost_plus_10pct_gcp_catalog",
            Self::Unavailable => "unavailable",
        }
    }
}

/// How the metered dimensions in a [`ComputeUsage`] record were observed
/// (`openagents.compute_quota_routing.v1`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeteringSource {
    /// Dimensions read from a GCP-reported metering surface.
    GcpReported,
    /// Dimensions measured directly on/around the node (e.g. lease wall-time).
    NodeMeasured,
    /// Dimensions estimated because no measured/reported source was available.
    Estimated,
}

impl MeteringSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GcpReported => "gcp_reported",
            Self::NodeMeasured => "node_measured",
            Self::Estimated => "estimated",
        }
    }
}

/// Infra compute metering + billing input sub-record
/// (`openagents.compute_quota_routing.v1` `compute_usage`).
///
/// Refs-and-limits only: this carries the *modeled/metered infra billing input*
/// (`cost_input_microusd`, the cost-plus-10% figure forwarded to Treasury), never
/// a customer's billed/settled amount, raw GCP invoice ids, or customer identity.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ComputeUsage {
    /// Compute capacity class id (e.g. [`GCE_EPHEMERAL_CAPACITY_CLASS_ID`]).
    pub compute_class: String,
    /// Metered VM wall-clock seconds. Measured from the lease lifecycle
    /// (`release_at − acquire_at`, whole seconds, saturating at 0).
    pub vm_seconds: u64,
    /// How `vm_seconds` (and any other dimensions) were observed.
    pub metering_source: MeteringSource,
    /// Nullable billing input: `floor(vm_seconds × cost-plus-10% rate)` in
    /// micro-USD. `None` only when `cost_input_basis = Unavailable`.
    pub cost_input_microusd: Option<u128>,
    /// How `cost_input_microusd` was formed.
    pub cost_input_basis: CostInputBasis,
}

impl ComputeUsage {
    /// Build a `compute_usage` record from a genuinely measured VM-second count
    /// and the catalog/list-price-derived GCE cost model. The cost-plus-10%
    /// markup is applied by [`LaneCostModel::gce_micro_usd_per_vm_sec`] — never
    /// re-derived inline — and the basis is the catalog variant because the rate
    /// comes from the published list-price catalog, not a live Billing export
    /// (cloud#92).
    pub fn gce_catalog_from_vm_seconds(
        compute_class: impl Into<String>,
        vm_seconds: u64,
        cost_model: &LaneCostModel,
    ) -> Self {
        let micro_usd_per_vm_sec = cost_model.gce_micro_usd_per_vm_sec();
        let cost_input_microusd = (vm_seconds as u128).saturating_mul(micro_usd_per_vm_sec as u128);
        Self {
            compute_class: compute_class.into(),
            vm_seconds,
            metering_source: MeteringSource::NodeMeasured,
            cost_input_microusd: Some(cost_input_microusd),
            cost_input_basis: CostInputBasis::CostPlus10pctGcpCatalog,
        }
    }

    fn validate_contract(&self) -> Result<(), String> {
        validate_contract_ref("compute_usage.compute_class", self.compute_class.as_str())?;
        match (self.cost_input_basis, self.cost_input_microusd) {
            (CostInputBasis::Unavailable, Some(_)) => {
                return Err(
                    "compute_usage cost_input_microusd must be null when basis is unavailable"
                        .to_string(),
                );
            }
            (CostInputBasis::Unavailable, None) => {}
            (_, None) => {
                return Err(
                    "compute_usage cost_input_microusd is required unless basis is unavailable"
                        .to_string(),
                );
            }
            (_, Some(_)) => {}
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModelUsageRecord {
    pub provider: String,
    pub backend: String,
    pub model: String,
    pub mode: String,
    pub account_ref: Option<String>,
    pub input_tokens: Option<u64>,
    pub cached_input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub count_source: TokenCountSource,
    pub cost_microusd: Option<u128>,
    pub billing_basis: String,
    pub unavailable_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenCountSource {
    ProviderReported,
    CodexReported,
    ParsedFromStream,
    Estimated,
    Unavailable,
}

impl ResourceUsageReceipt {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.schema_version != RESOURCE_USAGE_RECEIPT_VERSION {
            return Err(format!(
                "unexpected resource usage receipt version '{}'",
                self.schema_version
            ));
        }
        for (field, value) in [
            ("receipt_id", self.receipt_id.as_str()),
            ("run_ref", self.run_ref.as_str()),
            ("workroom_id", self.workroom_id.as_str()),
            ("node_ref", self.node_ref.as_str()),
            ("receipt_digest", self.receipt_digest.as_str()),
        ] {
            validate_contract_ref(field, value)?;
        }
        if !self.receipt_digest.starts_with("sha256:") {
            return Err("resource usage receipt digest must start with sha256:".to_string());
        }
        self.host.validate_contract()?;
        self.run.validate_contract()?;
        if self.model_usage.is_empty() {
            return Err(
                "resource usage receipt requires at least one model usage record".to_string(),
            );
        }
        for record in &self.model_usage {
            record.validate_contract()?;
        }
        if let Some(compute_usage) = &self.compute_usage {
            compute_usage.validate_contract()?;
        }
        Ok(())
    }
}

impl ResourceHostSnapshot {
    fn validate_contract(&self) -> Result<(), String> {
        for (field, value) in [
            ("host.os", self.os.as_str()),
            ("host.arch", self.arch.as_str()),
            ("host.cpu", self.cpu.as_str()),
        ] {
            validate_contract_ref(field, value)?;
        }
        if self.logical_cpu_count == 0 {
            return Err("resource host snapshot requires at least one logical CPU".to_string());
        }
        for accelerator in &self.accelerator_inventory {
            validate_contract_ref("host.accelerator_inventory", accelerator)?;
        }
        self.virtualization.validate_contract()
    }
}

impl VirtualizationFacts {
    fn validate_contract(&self) -> Result<(), String> {
        for (field, value) in [
            (
                "virtualization.container_runtime",
                self.container_runtime.as_deref(),
            ),
            ("virtualization.cgroup_mode", self.cgroup_mode.as_deref()),
        ] {
            if let Some(value) = value {
                validate_contract_ref(field, value)?;
            }
        }
        Ok(())
    }
}

impl RunResourceUsage {
    fn validate_contract(&self) -> Result<(), String> {
        validate_contract_ref("run.sandbox", self.sandbox.as_str())?;
        validate_digest_or_hex(
            "run.image_or_profile_digest",
            self.image_or_profile_digest.as_str(),
        )?;
        validate_digest_or_hex("run.workspace_digest", self.workspace_digest.as_str())?;
        Ok(())
    }
}

impl ModelUsageRecord {
    fn validate_contract(&self) -> Result<(), String> {
        for (field, value) in [
            ("model.provider", self.provider.as_str()),
            ("model.backend", self.backend.as_str()),
            ("model.model", self.model.as_str()),
            ("model.mode", self.mode.as_str()),
            ("model.billing_basis", self.billing_basis.as_str()),
        ] {
            validate_contract_ref(field, value)?;
        }
        if let Some(account_ref) = &self.account_ref {
            validate_contract_ref("model.account_ref", account_ref)?;
        }
        if let Some(reason) = &self.unavailable_reason {
            validate_contract_ref("model.unavailable_reason", reason)?;
        }
        if self.count_source == TokenCountSource::Unavailable
            && self
                .unavailable_reason
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
        {
            return Err("unavailable token counts require an unavailable_reason".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrainingRunAssignment {
    pub contract_version: String,
    pub training_run_id: String,
    pub benchmark_run_id: String,
    pub task_run_id: String,
    pub target_node_id: String,
    pub dataset: TrainingDatasetSelector,
    pub variants: Vec<TrainingRunVariant>,
    pub provider_account_ref: String,
    pub auth_grant_ref: String,
    pub repository_ref: Option<String>,
    pub signature_context: Option<TrainingSignatureContext>,
    pub codex_adapter: CodexPackageAdapter,
    pub budget: TrainingRunBudget,
    pub artifacts: TrainingArtifactPolicy,
    pub callback: Option<TrainingCallbackPolicy>,
    pub created_at_ms: u128,
    pub audit_context: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrainingDatasetSelector {
    pub dataset_slug: String,
    pub dataset_version: String,
    pub task_ref: String,
    pub task_checksum: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrainingRunVariant {
    pub variant_id: String,
    pub agent: String,
    pub model: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrainingSignatureContext {
    pub blueprint_signature_ids: Vec<String>,
    pub package_digest: Option<String>,
    pub selector_trace_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CodexPackageAdapter {
    pub adapter_id: String,
    pub package_name: String,
    pub package_version: String,
    pub package_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrainingRunBudget {
    pub timeout_ms: u128,
    pub max_attempts: u32,
    pub max_cost_microusd: Option<u128>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrainingArtifactPolicy {
    pub retention_mode: TrainingRetentionMode,
    pub artifact_sink_ref: Option<String>,
    pub required_artifacts: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRetentionMode {
    DurableArtifacts,
    RedactedOnly,
    LocalOnly,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrainingCallbackPolicy {
    pub callback_ref: String,
    pub event_sequence_policy: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtanisBootstrapAssignment {
    pub contract_version: String,
    pub bootstrap_run_id: String,
    pub workroom_id: String,
    pub target_node_id: String,
    pub operator_ref: String,
    pub organization_ref: Option<String>,
    pub provider_account_ref: String,
    pub auth_grant_ref: String,
    pub repository_refs: Vec<String>,
    pub source_refs: Vec<String>,
    pub objective_id: String,
    pub objective_summary: String,
    pub pylon_launch_id: String,
    pub settlement_intent: Option<ArtanisSettlementIntent>,
    pub pylon_capability_labels: Vec<String>,
    pub blueprint_signature_ids: Vec<String>,
    pub budget: TrainingRunBudget,
    pub retention_mode: TrainingRetentionMode,
    pub artifact_sink_ref: Option<String>,
    pub required_artifacts: Vec<String>,
    pub wallet_authority: bool,
    pub created_at_ms: u128,
    pub audit_context: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArtanisSettlementIntent {
    pub artanis_run_id: String,
    pub artanis_assignment_id: String,
    pub settlement_intent_id: String,
    pub public_receipt_id: Option<String>,
}

impl TrainingRunAssignment {
    pub fn validate_contract(&self, now_ms: u128) -> Result<(), String> {
        if self.contract_version != TRAINING_RUN_ASSIGNMENT_VERSION {
            return Err(format!(
                "unexpected training run assignment version '{}'",
                self.contract_version
            ));
        }
        for (field, value) in [
            ("training_run_id", self.training_run_id.as_str()),
            ("benchmark_run_id", self.benchmark_run_id.as_str()),
            ("task_run_id", self.task_run_id.as_str()),
            ("target_node_id", self.target_node_id.as_str()),
            ("provider_account_ref", self.provider_account_ref.as_str()),
            ("auth_grant_ref", self.auth_grant_ref.as_str()),
            ("audit_context", self.audit_context.as_str()),
        ] {
            validate_contract_ref(field, value)?;
        }
        if let Some(repository_ref) = &self.repository_ref {
            validate_contract_ref("repository_ref", repository_ref)?;
        }
        self.dataset.validate_contract()?;
        if self.variants.is_empty() {
            return Err("training run assignment requires at least one variant".to_string());
        }
        for variant in &self.variants {
            variant.validate_contract()?;
        }
        self.codex_adapter.validate_contract()?;
        self.budget.validate_contract()?;
        self.artifacts.validate_contract()?;
        if let Some(signature_context) = &self.signature_context {
            signature_context.validate_contract()?;
        }
        if let Some(callback) = &self.callback {
            callback.validate_contract()?;
        }
        if self.created_at_ms > now_ms + 1000 * 60 * 5 {
            return Err("training run assignment is from the future".to_string());
        }
        Ok(())
    }
}

impl ArtanisBootstrapAssignment {
    pub fn validate_contract(&self, now_ms: u128) -> Result<(), String> {
        if self.contract_version != ARTANIS_BOOTSTRAP_ASSIGNMENT_VERSION {
            return Err(format!(
                "unexpected Artanis bootstrap assignment version '{}'",
                self.contract_version
            ));
        }
        for (field, value) in [
            ("bootstrap_run_id", self.bootstrap_run_id.as_str()),
            ("workroom_id", self.workroom_id.as_str()),
            ("target_node_id", self.target_node_id.as_str()),
            ("operator_ref", self.operator_ref.as_str()),
            ("provider_account_ref", self.provider_account_ref.as_str()),
            ("auth_grant_ref", self.auth_grant_ref.as_str()),
            ("objective_id", self.objective_id.as_str()),
            ("pylon_launch_id", self.pylon_launch_id.as_str()),
            ("audit_context", self.audit_context.as_str()),
        ] {
            validate_contract_ref(field, value)?;
        }
        if let Some(organization_ref) = &self.organization_ref {
            validate_contract_ref("organization_ref", organization_ref)?;
        }
        if let Some(settlement_intent) = &self.settlement_intent {
            settlement_intent.validate_contract(self.bootstrap_run_id.as_str())?;
        }
        validate_public_text("objective_summary", &self.objective_summary)?;
        validate_contract_ref_vec("repository_ref", &self.repository_refs)?;
        validate_contract_ref_vec("source_ref", &self.source_refs)?;
        validate_contract_ref_vec("pylon_capability_label", &self.pylon_capability_labels)?;
        validate_contract_ref_vec("blueprint_signature_id", &self.blueprint_signature_ids)?;
        self.budget.validate_contract()?;
        if let Some(sink) = &self.artifact_sink_ref {
            validate_contract_ref("artifact_sink_ref", sink)?;
        }
        if self.required_artifacts.is_empty() {
            return Err("Artanis bootstrap assignment requires artifacts".to_string());
        }
        for artifact in &self.required_artifacts {
            validate_artifact_name(artifact)?;
        }
        if self.wallet_authority {
            return Err(
                "Artanis bootstrap workrooms must not receive wallet authority".to_string(),
            );
        }
        if self.created_at_ms > now_ms + 1000 * 60 * 5 {
            return Err("Artanis bootstrap assignment is from the future".to_string());
        }
        Ok(())
    }
}

impl ArtanisSettlementIntent {
    fn validate_contract(&self, bootstrap_run_id: &str) -> Result<(), String> {
        for (field, value) in [
            (
                "settlement_intent.artanis_run_id",
                self.artanis_run_id.as_str(),
            ),
            (
                "settlement_intent.artanis_assignment_id",
                self.artanis_assignment_id.as_str(),
            ),
            (
                "settlement_intent.settlement_intent_id",
                self.settlement_intent_id.as_str(),
            ),
        ] {
            validate_contract_ref(field, value)?;
        }
        if self.artanis_run_id != bootstrap_run_id {
            return Err("settlement intent Artanis run id must match bootstrap run id".to_string());
        }
        if let Some(public_receipt_id) = &self.public_receipt_id {
            validate_contract_ref("settlement_intent.public_receipt_id", public_receipt_id)?;
        }
        Ok(())
    }
}

impl TrainingDatasetSelector {
    fn validate_contract(&self) -> Result<(), String> {
        validate_contract_ref("dataset_slug", self.dataset_slug.as_str())?;
        validate_contract_ref("dataset_version", self.dataset_version.as_str())?;
        validate_contract_ref("task_ref", self.task_ref.as_str())?;
        if self.dataset_slug != "terminal-bench" {
            return Err(
                "first training assignment runner only supports terminal-bench".to_string(),
            );
        }
        if !self.task_ref.starts_with("terminal-bench/") {
            return Err("terminal-bench task_ref must be a registry task ref".to_string());
        }
        if let Some(checksum) = &self.task_checksum {
            validate_digest_or_hex("task_checksum", checksum)?;
        }
        Ok(())
    }
}

impl TrainingRunVariant {
    fn validate_contract(&self) -> Result<(), String> {
        validate_contract_ref("variant_id", self.variant_id.as_str())?;
        validate_contract_ref("agent", self.agent.as_str())?;
        validate_contract_ref("model", self.model.as_str())?;
        Ok(())
    }
}

impl TrainingSignatureContext {
    fn validate_contract(&self) -> Result<(), String> {
        if self.blueprint_signature_ids.is_empty() {
            return Err("signature context requires at least one signature id".to_string());
        }
        for signature_id in &self.blueprint_signature_ids {
            validate_contract_ref("blueprint_signature_id", signature_id)?;
        }
        if let Some(package_digest) = &self.package_digest {
            validate_digest_or_hex("signature package digest", package_digest)?;
        }
        Ok(())
    }
}

impl CodexPackageAdapter {
    fn validate_contract(&self) -> Result<(), String> {
        validate_contract_ref("codex adapter id", self.adapter_id.as_str())?;
        validate_contract_ref("codex package name", self.package_name.as_str())?;
        validate_contract_ref("codex package version", self.package_version.as_str())?;
        if let Some(package_digest) = &self.package_digest {
            validate_digest_or_hex("codex package digest", package_digest)?;
        }
        Ok(())
    }
}

impl TrainingRunBudget {
    fn validate_contract(&self) -> Result<(), String> {
        if self.timeout_ms == 0 || self.timeout_ms > 1000 * 60 * 60 {
            return Err("training run timeout must be between 1ms and 1h".to_string());
        }
        if self.max_attempts == 0 || self.max_attempts > 3 {
            return Err("training run max_attempts must be between 1 and 3".to_string());
        }
        Ok(())
    }
}

impl TrainingArtifactPolicy {
    fn validate_contract(&self) -> Result<(), String> {
        if self.required_artifacts.is_empty() {
            return Err("training assignment requires at least one artifact".to_string());
        }
        for artifact in &self.required_artifacts {
            validate_artifact_name(artifact)?;
        }
        if let Some(sink) = &self.artifact_sink_ref {
            validate_contract_ref("artifact_sink_ref", sink)?;
        }
        Ok(())
    }
}

impl TrainingCallbackPolicy {
    fn validate_contract(&self) -> Result<(), String> {
        validate_contract_ref("callback_ref", self.callback_ref.as_str())?;
        validate_contract_ref("event_sequence_policy", self.event_sequence_policy.as_str())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CloudNodeSnapshot {
    pub contract_version: String,
    pub identity: NodeIdentity,
    pub host: HostFacts,
    pub lifecycle: NodeLifecycle,
    pub capabilities: NodeCapabilities,
    pub policy: NodePolicy,
    pub runtime: NodeRuntime,
    pub evidence: NodeEvidence,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeIdentity {
    pub node_id: String,
    pub operator_identity: String,
    pub account_or_org_binding: String,
    pub signing_key_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HostFacts {
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub memory: String,
    pub disk: String,
    pub accelerator_inventory: Vec<String>,
    pub site_or_power_metadata: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeLifecycle {
    pub desired_mode: DesiredMode,
    pub observed_status: ObservedStatus,
    pub degradation_reason: Option<String>,
    pub service_manager: String,
    pub update_channel: String,
    pub last_started_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesiredMode {
    Offline,
    Online,
    Paused,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservedStatus {
    Unconfigured,
    Offline,
    Online,
    Degraded,
    Quarantined,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub inference_products: Vec<ProductCapability>,
    pub training_products: Vec<ProductCapability>,
    pub sandbox_profiles: Vec<SandboxProfileSummary>,
    pub workroom_capacity: Option<WorkroomCapacity>,
    pub ingress_support: CapabilityState,
    pub artifact_support: CapabilityState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProductCapability {
    pub product_id: String,
    pub enabled: bool,
    pub backend_ready: bool,
    pub eligible: bool,
    pub capability_summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SandboxProfileSummary {
    pub profile_id: String,
    pub profile_digest: String,
    pub execution_class: String,
    pub ready: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkroomCapacity {
    pub max_active_workrooms: u32,
    pub default_runtime_profile: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityState {
    pub supported: bool,
    pub enabled: bool,
    pub ready: bool,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodePolicy {
    pub accepted_work_policy: String,
    pub sandbox_policy: String,
    pub network_policy: String,
    pub filesystem_policy: String,
    pub secret_policy: String,
    pub settlement_policy: SettlementPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementPolicy {
    ContributorWallet,
    InternalAccounting,
    NoWallet,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeRuntime {
    pub local_admin_endpoint: Option<String>,
    pub heartbeat_endpoint: Option<String>,
    pub job_intake_modes: Vec<String>,
    pub receipt_sink: String,
    pub artifact_sink: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NodeEvidence {
    pub current_snapshot_digest: String,
    pub health_events: Vec<String>,
    pub job_receipts: Vec<String>,
    pub artifact_receipts: Vec<String>,
    pub payout_or_accounting_receipts: Vec<String>,
}

impl CloudNodeSnapshot {
    pub fn managed_scaffold() -> Self {
        Self {
            contract_version: CLOUD_NODE_CONTRACT_VERSION.to_string(),
            identity: NodeIdentity {
                node_id: "node.managed.local.scaffold".to_string(),
                operator_identity: "org.local.scaffold".to_string(),
                account_or_org_binding: "org.local.scaffold".to_string(),
                signing_key_ref: "local-dev-key-ref".to_string(),
            },
            host: HostFacts {
                os: std::env::consts::OS.to_string(),
                arch: std::env::consts::ARCH.to_string(),
                cpu: "unknown".to_string(),
                memory: "unknown".to_string(),
                disk: "unknown".to_string(),
                accelerator_inventory: Vec::new(),
                site_or_power_metadata: None,
            },
            lifecycle: NodeLifecycle {
                desired_mode: DesiredMode::Offline,
                observed_status: ObservedStatus::Unconfigured,
                degradation_reason: Some("managed runtime not initialized".to_string()),
                service_manager: "none".to_string(),
                update_channel: "local-dev".to_string(),
                last_started_at: None,
                last_heartbeat_at: None,
            },
            capabilities: NodeCapabilities {
                inference_products: Vec::new(),
                training_products: Vec::new(),
                sandbox_profiles: Vec::new(),
                workroom_capacity: Some(WorkroomCapacity {
                    max_active_workrooms: 0,
                    default_runtime_profile: "not_configured".to_string(),
                }),
                ingress_support: CapabilityState {
                    supported: false,
                    enabled: false,
                    ready: false,
                    detail: Some("not implemented".to_string()),
                },
                artifact_support: CapabilityState {
                    supported: false,
                    enabled: false,
                    ready: false,
                    detail: Some("not implemented".to_string()),
                },
            },
            policy: NodePolicy {
                accepted_work_policy: "managed-cloud-mvp".to_string(),
                sandbox_policy: "disabled_until_profiled".to_string(),
                network_policy: "deny_by_default".to_string(),
                filesystem_policy: "workroom_scoped".to_string(),
                secret_policy: "brokered_no_raw_secrets".to_string(),
                settlement_policy: SettlementPolicy::NoWallet,
            },
            runtime: NodeRuntime {
                local_admin_endpoint: None,
                heartbeat_endpoint: None,
                job_intake_modes: vec!["forge_typed_assignment".to_string()],
                receipt_sink: "local-dev".to_string(),
                artifact_sink: "local-dev".to_string(),
            },
            evidence: NodeEvidence {
                current_snapshot_digest: "sha256:not-computed-scaffold".to_string(),
                health_events: Vec::new(),
                job_receipts: Vec::new(),
                artifact_receipts: Vec::new(),
                payout_or_accounting_receipts: Vec::new(),
            },
        }
    }

    pub fn validate_contract(&self) -> Result<(), String> {
        if self.contract_version != CLOUD_NODE_CONTRACT_VERSION {
            return Err(format!(
                "unexpected contract version '{}'",
                self.contract_version
            ));
        }
        if self.identity.node_id.trim().is_empty() {
            return Err("node_id must not be empty".to_string());
        }
        if self.identity.operator_identity.trim().is_empty() {
            return Err("operator_identity must not be empty".to_string());
        }
        if self.lifecycle.observed_status == ObservedStatus::Online
            && self.lifecycle.degradation_reason.is_some()
        {
            return Err("online nodes must not carry degradation_reason".to_string());
        }
        if self.policy.settlement_policy != SettlementPolicy::ContributorWallet
            && self
                .evidence
                .payout_or_accounting_receipts
                .iter()
                .any(|receipt| receipt.contains("wallet_seed"))
        {
            return Err("managed/no-wallet nodes must not expose wallet seed evidence".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkroomSnapshot {
    pub contract_version: String,
    pub identity: WorkroomIdentity,
    pub lifecycle: WorkroomLifecycle,
    pub runtime: WorkroomRuntime,
    pub capabilities: Vec<CapabilityAttachment>,
    pub local_gateways: LocalGateways,
    pub ingress: WorkroomIngress,
    pub receipts: WorkroomReceipts,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkroomIdentity {
    pub workroom_id: String,
    pub org_id: String,
    pub program_id: String,
    pub template_id: String,
    pub node_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkroomLifecycle {
    pub desired_state: WorkroomDesiredState,
    pub observed_state: WorkroomObservedState,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub closeout_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkroomDesiredState {
    Created,
    Running,
    Paused,
    Closing,
    Archived,
    Destroyed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkroomObservedState {
    Created,
    Running,
    Paused,
    Degraded,
    Closed,
    Archived,
    Destroyed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkroomRuntime {
    pub runtime_kind: String,
    pub image_or_profile_digest: String,
    pub workspace_digest: String,
    pub resource_limits: String,
    pub network_policy: String,
    pub filesystem_policy: String,
    pub timeout_policy: String,
    pub wallet_authority: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CapabilityAttachment {
    pub capability: String,
    pub scope: String,
    pub enabled: bool,
    pub ready: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalGateways {
    pub model: String,
    pub artifacts: String,
    pub receipts: String,
    pub memory: String,
    pub email: String,
    pub settlement: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkroomIngress {
    pub visibility: IngressVisibility,
    pub preview_url: Option<String>,
    pub endpoint_tokens: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngressVisibility {
    Private,
    Collaborators,
    Public,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkroomReceipts {
    pub events: Vec<WorkroomReceiptEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkroomReceiptEvent {
    pub event_id: String,
    pub event_kind: WorkroomReceiptKind,
    pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkroomReceiptKind {
    Created,
    CapabilityAttached,
    PreviewExposed,
    TokenMinted,
    ArtifactUploaded,
    CloseoutSubmitted,
    DestroyedOrArchived,
}

impl WorkroomSnapshot {
    pub fn scaffold() -> Self {
        Self {
            contract_version: WORKROOM_CONTRACT_VERSION.to_string(),
            identity: WorkroomIdentity {
                workroom_id: "workroom.local.scaffold".to_string(),
                org_id: "org.local.scaffold".to_string(),
                program_id: "program.local.scaffold".to_string(),
                template_id: "template.local.scaffold".to_string(),
                node_id: "node.managed.local.scaffold".to_string(),
            },
            lifecycle: WorkroomLifecycle {
                desired_state: WorkroomDesiredState::Created,
                observed_state: WorkroomObservedState::Created,
                created_at: None,
                updated_at: None,
                closeout_required: true,
            },
            runtime: WorkroomRuntime {
                runtime_kind: "not_configured".to_string(),
                image_or_profile_digest: "sha256:not-configured".to_string(),
                workspace_digest: "sha256:not-configured".to_string(),
                resource_limits: "not_configured".to_string(),
                network_policy: "deny_by_default".to_string(),
                filesystem_policy: "workroom_scoped".to_string(),
                timeout_policy: "not_configured".to_string(),
                wallet_authority: false,
            },
            capabilities: Vec::new(),
            local_gateways: LocalGateways::default_paths(),
            ingress: WorkroomIngress {
                visibility: IngressVisibility::Private,
                preview_url: None,
                endpoint_tokens: Vec::new(),
            },
            receipts: WorkroomReceipts {
                events: vec![WorkroomReceiptEvent {
                    event_id: "evt.workroom.created.scaffold".to_string(),
                    event_kind: WorkroomReceiptKind::Created,
                    digest: "sha256:workroom-created-scaffold".to_string(),
                }],
            },
        }
    }

    pub fn validate_contract(&self) -> Result<(), String> {
        if self.contract_version != WORKROOM_CONTRACT_VERSION {
            return Err(format!(
                "unexpected workroom contract version '{}'",
                self.contract_version
            ));
        }
        if self.identity.workroom_id.trim().is_empty() {
            return Err("workroom_id must not be empty".to_string());
        }
        if self.runtime.wallet_authority {
            return Err("workrooms must not receive wallet authority by default".to_string());
        }
        if self.ingress.visibility != IngressVisibility::Private
            && !self
                .receipts
                .events
                .iter()
                .any(|event| event.event_kind == WorkroomReceiptKind::PreviewExposed)
        {
            return Err("non-private ingress requires preview_exposed receipt".to_string());
        }
        if !self.capabilities.is_empty()
            && !self
                .receipts
                .events
                .iter()
                .any(|event| event.event_kind == WorkroomReceiptKind::CapabilityAttached)
        {
            return Err("capability attachments require capability_attached receipt".to_string());
        }
        self.local_gateways.validate_paths()
    }
}

impl LocalGateways {
    pub fn default_paths() -> Self {
        Self {
            model: "/openagents/model".to_string(),
            artifacts: "/openagents/artifacts".to_string(),
            receipts: "/openagents/receipts".to_string(),
            memory: "/openagents/memory".to_string(),
            email: "/openagents/email".to_string(),
            settlement: "/openagents/settlement".to_string(),
        }
    }

    fn validate_paths(&self) -> Result<(), String> {
        for path in [
            &self.model,
            &self.artifacts,
            &self.receipts,
            &self.memory,
            &self.email,
            &self.settlement,
        ] {
            if !path.starts_with("/openagents/") {
                return Err(format!(
                    "local gateway path '{path}' must start with /openagents/"
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForgeAssignment {
    pub contract_version: String,
    pub assignment_id: String,
    pub org_id: String,
    pub program_id: String,
    pub workroom_id: String,
    pub node_id: Option<String>,
    pub assignment_kind: ForgeAssignmentKind,
    pub template: ForgeAssignmentTemplate,
    pub capability: ForgeAssignmentCapability,
    pub budget: ForgeAssignmentBudgetPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<ForgeAssignmentSandboxPolicy>,
    pub artifacts: ForgeAssignmentArtifactPolicy,
    pub receipts: ForgeAssignmentReceiptPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgeAssignmentKind {
    Workroom,
    Worker,
    OpenEndedLabor,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForgeAssignmentTemplate {
    pub template_id: String,
    pub runtime_profile: String,
    pub template_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForgeAssignmentCapability {
    pub capability_id: String,
    pub capability_scope: String,
    pub required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForgeAssignmentBudgetPolicy {
    pub max_runtime_ms: u64,
    pub max_cost_microusd: u64,
    pub max_artifact_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForgeAssignmentSandboxPolicy {
    pub profile_id: String,
    pub profile_digest: String,
    pub execution_class: String,
    pub network_policy: String,
    pub filesystem_policy: String,
    pub secret_policy: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForgeAssignmentArtifactPolicy {
    pub artifact_sink: String,
    pub required_artifacts: Vec<String>,
    pub retention: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForgeAssignmentReceiptPolicy {
    pub receipt_sink: String,
    pub required_receipts: Vec<String>,
    pub closeout_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ForgeAssignmentReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub assignment_id: String,
    pub node_id: String,
    pub decision: ForgeAssignmentDecision,
    pub reason: String,
    pub assignment_digest: String,
    pub receipt_digest: String,
    pub emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForgeAssignmentDecision {
    Accepted,
    Refused,
}

impl ForgeAssignment {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.contract_version != FORGE_ASSIGNMENT_CONTRACT_VERSION {
            return Err(format!(
                "unexpected forge assignment contract version '{}'",
                self.contract_version
            ));
        }
        for (field, value) in [
            ("assignment_id", self.assignment_id.as_str()),
            ("org_id", self.org_id.as_str()),
            ("program_id", self.program_id.as_str()),
            ("workroom_id", self.workroom_id.as_str()),
            ("template_id", self.template.template_id.as_str()),
            ("runtime_profile", self.template.runtime_profile.as_str()),
            ("template_digest", self.template.template_digest.as_str()),
            ("capability_id", self.capability.capability_id.as_str()),
            (
                "capability_scope",
                self.capability.capability_scope.as_str(),
            ),
            ("artifact_sink", self.artifacts.artifact_sink.as_str()),
            ("receipt_sink", self.receipts.receipt_sink.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
        }
        if !self.template.template_digest.starts_with("sha256:") {
            return Err("template_digest must be a sha256 digest reference".to_string());
        }
        if self.budget.max_runtime_ms == 0 {
            return Err("budget.max_runtime_ms must be greater than zero".to_string());
        }
        if self.budget.max_artifact_bytes == 0 {
            return Err("budget.max_artifact_bytes must be greater than zero".to_string());
        }
        if self.receipts.required_receipts.is_empty() {
            return Err("receipt policy must require at least one receipt".to_string());
        }
        if self.assignment_kind == ForgeAssignmentKind::Worker
            && self.capability.capability_id.starts_with("sandbox.")
            && self.sandbox.is_none()
        {
            return Err("sandbox worker assignments must declare sandbox policy".to_string());
        }
        if let Some(sandbox) = &self.sandbox {
            for (field, value) in [
                ("sandbox.profile_id", sandbox.profile_id.as_str()),
                ("sandbox.profile_digest", sandbox.profile_digest.as_str()),
                ("sandbox.execution_class", sandbox.execution_class.as_str()),
                ("sandbox.network_policy", sandbox.network_policy.as_str()),
                (
                    "sandbox.filesystem_policy",
                    sandbox.filesystem_policy.as_str(),
                ),
                ("sandbox.secret_policy", sandbox.secret_policy.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(format!("{field} must not be empty"));
                }
            }
            if !sandbox.profile_digest.starts_with("sha256:") {
                return Err("sandbox.profile_digest must be a sha256 digest reference".to_string());
            }
        }
        Ok(())
    }
}

impl ForgeAssignmentReceipt {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.schema_version != FORGE_ASSIGNMENT_RECEIPT_VERSION {
            return Err(format!(
                "unexpected forge assignment receipt version '{}'",
                self.schema_version
            ));
        }
        for (field, value) in [
            ("receipt_id", self.receipt_id.as_str()),
            ("assignment_id", self.assignment_id.as_str()),
            ("node_id", self.node_id.as_str()),
            ("reason", self.reason.as_str()),
            ("assignment_digest", self.assignment_digest.as_str()),
            ("receipt_digest", self.receipt_digest.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
        }
        if !self.assignment_digest.starts_with("sha256:") {
            return Err("assignment_digest must be a sha256 digest reference".to_string());
        }
        if !self.receipt_digest.starts_with("sha256:") {
            return Err("receipt_digest must be a sha256 digest reference".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PsionicWorkerAttachment {
    pub schema_version: String,
    pub updated_at_ms: u128,
    pub workers: Vec<PsionicWorkerState>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PsionicWorkerState {
    pub product_id: String,
    pub worker_id: String,
    pub worker_kind: PsionicWorkerKind,
    pub ready: bool,
    pub crashed: bool,
    pub evidence_digest: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicWorkerKind {
    Inference,
    Training,
    Sandbox,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PsionicExecutionReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub assignment_id: String,
    pub product_id: String,
    pub worker_id: String,
    pub status: PsionicExecutionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_digest: Option<String>,
    pub psionic_evidence_digest: String,
    pub receipt_digest: String,
    pub emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionicExecutionStatus {
    Succeeded,
    Failed,
    Refused,
}

impl PsionicWorkerAttachment {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.schema_version != PSIONIC_WORKER_ATTACHMENT_VERSION {
            return Err(format!(
                "unexpected psionic worker attachment version '{}'",
                self.schema_version
            ));
        }
        if self.workers.is_empty() {
            return Err("psionic worker attachment must contain at least one worker".to_string());
        }
        for worker in &self.workers {
            for (field, value) in [
                ("product_id", worker.product_id.as_str()),
                ("worker_id", worker.worker_id.as_str()),
            ] {
                if value.trim().is_empty() {
                    return Err(format!("{field} must not be empty"));
                }
            }
            if worker.ready && worker.crashed {
                return Err(format!(
                    "psionic worker '{}' cannot be both ready and crashed",
                    worker.worker_id
                ));
            }
            if let Some(digest) = &worker.evidence_digest {
                if !digest.starts_with("sha256:") {
                    return Err(format!(
                        "psionic worker '{}' evidence_digest must be sha256",
                        worker.worker_id
                    ));
                }
            }
        }
        Ok(())
    }
}

impl PsionicExecutionReceipt {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.schema_version != PSIONIC_EXECUTION_RECEIPT_VERSION {
            return Err(format!(
                "unexpected psionic execution receipt version '{}'",
                self.schema_version
            ));
        }
        for (field, value) in [
            ("receipt_id", self.receipt_id.as_str()),
            ("assignment_id", self.assignment_id.as_str()),
            ("product_id", self.product_id.as_str()),
            ("worker_id", self.worker_id.as_str()),
            (
                "psionic_evidence_digest",
                self.psionic_evidence_digest.as_str(),
            ),
            ("receipt_digest", self.receipt_digest.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
        }
        if !self.psionic_evidence_digest.starts_with("sha256:") {
            return Err("psionic_evidence_digest must be a sha256 digest reference".to_string());
        }
        if !self.receipt_digest.starts_with("sha256:") {
            return Err("receipt_digest must be a sha256 digest reference".to_string());
        }
        if let Some(profile_digest) = &self.profile_digest {
            if !profile_digest.starts_with("sha256:") {
                return Err("profile_digest must be a sha256 digest reference".to_string());
            }
        }
        if self.product_id.starts_with("sandbox.") && self.profile_digest.is_none() {
            return Err("sandbox execution receipts must cite profile_digest".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeWorkerAttachment {
    pub schema_version: String,
    pub workroom_id: String,
    pub program_id: String,
    pub worker_id: String,
    pub workspace_root: String,
    pub capability_names: Vec<String>,
    pub raw_secret_access: bool,
    pub secret_refs: Vec<String>,
    pub artifact_dir: String,
    pub receipt_sink: String,
    pub updated_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProbeCloseoutReceipt {
    pub schema_version: String,
    pub receipt_id: String,
    pub workroom_id: String,
    pub worker_id: String,
    pub status: ProbeCloseoutStatus,
    pub artifact_refs: Vec<String>,
    pub receipt_digest: String,
    pub emitted_at_ms: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProbeCloseoutStatus {
    Succeeded,
    Failed,
    Refused,
}

impl ProbeWorkerAttachment {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.schema_version != PROBE_WORKER_ATTACHMENT_VERSION {
            return Err(format!(
                "unexpected probe worker attachment version '{}'",
                self.schema_version
            ));
        }
        for (field, value) in [
            ("workroom_id", self.workroom_id.as_str()),
            ("program_id", self.program_id.as_str()),
            ("worker_id", self.worker_id.as_str()),
            ("workspace_root", self.workspace_root.as_str()),
            ("artifact_dir", self.artifact_dir.as_str()),
            ("receipt_sink", self.receipt_sink.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
        }
        if self.raw_secret_access {
            return Err("probe worker attachments must not grant raw secret access".to_string());
        }
        if self.capability_names.is_empty() {
            return Err("probe worker attachment must name at least one capability".to_string());
        }
        if self
            .secret_refs
            .iter()
            .any(|secret_ref| looks_like_raw_secret(secret_ref.as_str()))
        {
            return Err("probe worker attachment contains a raw-looking secret ref".to_string());
        }
        Ok(())
    }
}

impl ProbeCloseoutReceipt {
    pub fn validate_contract(&self) -> Result<(), String> {
        if self.schema_version != PROBE_CLOSEOUT_RECEIPT_VERSION {
            return Err(format!(
                "unexpected probe closeout receipt version '{}'",
                self.schema_version
            ));
        }
        for (field, value) in [
            ("receipt_id", self.receipt_id.as_str()),
            ("workroom_id", self.workroom_id.as_str()),
            ("worker_id", self.worker_id.as_str()),
            ("receipt_digest", self.receipt_digest.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
        }
        if self.artifact_refs.is_empty() {
            return Err("probe closeout receipt must cite at least one artifact".to_string());
        }
        if !self.receipt_digest.starts_with("sha256:") {
            return Err("receipt_digest must be a sha256 digest reference".to_string());
        }
        if self
            .artifact_refs
            .iter()
            .any(|artifact| looks_like_raw_secret(artifact.as_str()))
        {
            return Err("probe closeout artifact refs must not contain raw secrets".to_string());
        }
        Ok(())
    }
}

fn looks_like_raw_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("sk-")
        || lower.contains("secret-token")
        || lower.contains("bearer ")
        || lower.contains("api_key=")
        || lower.contains("password=")
}

fn contains_secret_material(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "secret-token",
        "bearer ",
        "api_key",
        "openai_api_key",
        "access_token",
        "refresh_token",
        "id_token",
        "device_code",
        "code_verifier",
        "auth.json",
        "password",
        "wallet_seed",
        "private_key",
        "tailscale",
        "tailnet",
        "private_topology",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn validate_contract_ref(field: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} must not be empty"));
    }
    if trimmed.len() > 240 || contains_secret_material(trimmed) {
        return Err(format!("{field} is invalid"));
    }
    if !trimmed.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(ch, '-' | '_' | '.' | ':' | '/' | '@' | '+' | '=' | '#')
    }) {
        return Err(format!("{field} contains unsupported characters"));
    }
    Ok(())
}

fn validate_contract_ref_vec(field: &str, values: &[String]) -> Result<(), String> {
    if values.is_empty() {
        return Err(format!("{field} list must not be empty"));
    }
    for value in values {
        validate_contract_ref(field, value)?;
    }
    Ok(())
}

fn validate_public_text(field: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 1000 || contains_secret_material(trimmed) {
        return Err(format!("{field} is invalid"));
    }
    Ok(())
}

fn validate_digest_or_hex(field: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    let digest = trimmed.strip_prefix("sha256:").unwrap_or(trimmed);
    if digest.len() != 64 || !digest.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("{field} must be a sha256 digest or 64 hex chars"));
    }
    Ok(())
}

fn validate_artifact_name(value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 120
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || contains_secret_material(trimmed)
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.')
    {
        return Err("training artifact names must be bounded local filenames".to_string());
    }
    Ok(())
}

/// Lane-agnostic compute lane selector for a coding-run placement request.
///
/// `Auto` defers to fleet policy: per the owner direction (2026-06-14) the
/// cloud lane priority is Google GCE first, SHC second, so `Auto` binds to GCE
/// and only falls back to SHC when GCE is unavailable. The other variants are
/// explicit caller pins.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComputeLane {
    /// Cost-driven (CND-042): own-Pylon-first-and-free upstream, then the
    /// cheaper of GCE/SHC by cost-plus-10% (GCE wins ties/near-ties; SHC only
    /// when materially cheaper and the pilot recommends "expand").
    Auto,
    /// Caller's own local Pylon. Resolution is out of scope for the cloud
    /// placement endpoint; cloud placement treats this as a non-cloud lane.
    Local,
    /// Google GCE ephemeral-per-session VM (primary cloud lane).
    CloudGcp,
    /// SHC `oa-shc-katy-01` node (secondary cloud lane).
    CloudShc,
}

impl ComputeLane {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Local => "local",
            Self::CloudGcp => "cloud-gcp",
            Self::CloudShc => "cloud-shc",
        }
    }
}

impl Default for ComputeLane {
    fn default() -> Self {
        Self::Auto
    }
}

/// Quota and lifetime caps applied to a placement, sourced from
/// `openagents.compute_quota_routing.v1` defaults unless overridden by fleet
/// policy. Caps are refs/limits only; no cost, identity, or topology material.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ComputeQuotaCaps {
    pub session_ttl_ms: u128,
    pub idle_timeout_ms: u128,
    pub lease_ttl_ms: u128,
    pub pause_ttl_ms: u128,
    pub owner_active_session_cap: u32,
    pub owner_remote_lease_cap: u32,
}

impl Default for ComputeQuotaCaps {
    fn default() -> Self {
        Self {
            session_ttl_ms: DEFAULT_SESSION_TTL_MS,
            idle_timeout_ms: DEFAULT_IDLE_TIMEOUT_MS,
            lease_ttl_ms: DEFAULT_LEASE_TTL_MS,
            pause_ttl_ms: DEFAULT_PAUSE_TTL_MS,
            owner_active_session_cap: DEFAULT_OWNER_ACTIVE_SESSION_CAP,
            owner_remote_lease_cap: DEFAULT_OWNER_REMOTE_LEASE_CAP,
        }
    }
}

/// Lane-agnostic coding-run placement assignment posted by a generic control
/// front door (e.g. Pylon). It carries the same bounded run intent as a Codex
/// VM run, plus a `lane` selector, and is independent of any Vortex-shaped
/// caller (cloud#86).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlacementAssignment {
    pub contract_version: String,
    /// Stable run id assigned by the caller.
    pub run_id: String,
    /// Redacted owner ref used for per-owner quota evaluation.
    pub owner_ref: String,
    /// Sanitized ChatGPT/Codex provider-account ref.
    pub provider_account_ref: String,
    /// Session grant ref produced by `openagents.codex_auth_grant.v1`.
    pub auth_grant_ref: String,
    /// Bounded instruction for the coding run.
    pub goal: String,
    /// Requested compute lane. Defaults to `Auto` (GCE primary, SHC fallback).
    #[serde(default)]
    pub lane: ComputeLane,
    /// Non-secret repo/project context.
    pub repository: Option<String>,
    /// Sandbox profile; defaults to `danger_full_access` inside the no-wallet
    /// VM boundary, consistent with CND-041/CND-055 (cloud#88).
    pub sandbox_mode: Option<String>,
    pub wallet_authority: bool,
    pub created_at_ms: u128,
    /// AC-1 (openagents#8503): optional org-cloud-runtime work-context, base64 of
    /// the `openagents.agent_computer.work_context` JSON the in-guest turn-runner
    /// consumes (repo/commit + an optional `inference` block). When present with
    /// `lane = cloud-gcp` AND a live Firecracker cloud-vm provisioner, the daemon
    /// runs the turn INSIDE a microVM (`cloud_vm::run_cloud_vm_session`) instead
    /// of the Codex runner. It is an OPAQUE, execution-time blob: the daemon never
    /// parses or logs it, only writes it into the guest for the turn-runner to
    /// decode. Absent it, placement behaves exactly as before (Codex path). The
    /// blob may carry an execution-time agent bearer for the guest's hosted call;
    /// it is never retained in a receipt, event, or log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_context_b64: Option<String>,
    /// AC-1 (openagents#8503): public-safe work-context ref echoed back on the
    /// microVM lifecycle events so the caller's isolation-posture validator can
    /// confirm the placement bound the SAME work context it requested. Non-secret
    /// ref only (e.g. `work-context.agent-computer.<id>`); never a credential.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_context_ref: Option<String>,
}

/// Why a particular runner was selected.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlacementReason {
    /// Caller explicitly pinned this lane.
    LanePinned,
    /// Policy default selected GCE as the primary cloud lane.
    PolicyDefaultGce,
    /// GCE was unavailable; SHC selected as the secondary lane.
    GceUnavailableShcFallback,
    /// Cost-driven comparison selected this lane (CND-042). The chosen lane's
    /// cost-plus-10% basis is recorded in `RunnerBinding::cost_basis`. Per owner
    /// direction, GCE wins ties and near-ties; SHC is chosen only when it is
    /// materially cheaper AND the pilot recommendation is "expand".
    CostDriven,
}

/// Refs-and-limits-only cost basis recorded on a cost-driven binding (CND-042).
///
/// This surfaces the chosen lane's cost-plus-10% estimate and the comparison
/// inputs so downstream metering/settlement can reconcile, without exposing raw
/// customer cost in public-facing refs. The values here are the *modeled lane
/// estimate* (infra cost-plus-10% per the contract), never a customer's billed
/// amount, and never raw GCP/SHC invoice identifiers.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlacementCostBasis {
    /// `cost_plus_10pct_gcp` style label describing how the estimate was formed.
    pub basis: String,
    /// Chosen lane's cost-plus-10% estimate, micro-USD per VM-second.
    pub chosen_micro_usd_per_vm_sec: u64,
    /// GCE lane's cost-plus-10% estimate, micro-USD per VM-second.
    pub gce_micro_usd_per_vm_sec: u64,
    /// SHC lane's cost-plus-10% estimate, micro-USD per VM-second.
    pub shc_micro_usd_per_vm_sec: u64,
    /// Modeled session length used for the per-session figure below.
    pub modeled_session_vm_seconds: u64,
    /// Chosen lane's per-session cost-plus-10% estimate, micro-USD.
    pub chosen_session_micro_usd: u64,
}

/// Result of binding a lane-agnostic assignment to a concrete runner.
///
/// Placement is cost-driven for non-pinned `Auto` assignments once CND-042
/// receipt comparison landed: `cost_driven = true` with a `cost_basis` recorded
/// when the lane was chosen by cost comparison. Caller pins and the
/// policy-driven fallback path record `cost_driven = false` with `cost_basis`
/// absent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RunnerBinding {
    pub contract_version: String,
    pub run_id: String,
    pub external_run_id: String,
    pub lane: ComputeLane,
    pub provider_lane: ProviderLane,
    pub runner_id: String,
    /// Capacity class id for cloud lanes (the GCE C-5 class for GCE).
    pub capacity_class_id: Option<String>,
    pub sandbox_mode: String,
    pub reason: PlacementReason,
    /// True when the lane was selected by the CND-042 cost comparison.
    pub cost_driven: bool,
    /// Cost basis recorded when `cost_driven` is true; `None` otherwise. Refs
    /// and limits only — never raw customer cost.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_basis: Option<PlacementCostBasis>,
    pub caps: ComputeQuotaCaps,
}

/// Per-lane cost model for cost-driven placement (CND-042).
///
/// Holds the raw (pre-markup) per-VM-second rate for each cloud lane. The
/// cost-plus-10% markup from `openagents.compute_quota_routing.v1` is applied by
/// the accessor methods so the markup lives in exactly one place. Rates are
/// sourced from the CND-042 report; update the `*_RAW_PER_VM_SEC_NANOUSD`
/// constants to change them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaneCostModel {
    /// GCE raw rate, nano-USD per VM-second.
    pub gce_raw_nanousd_per_vm_sec: u64,
    /// SHC raw rate, nano-USD per VM-second.
    pub shc_raw_nanousd_per_vm_sec: u64,
    /// Modeled default session length, VM-seconds.
    pub session_vm_seconds: u64,
    /// Material-cheaper margin in basis points a challenger must beat GCE by.
    pub material_margin_bps: u64,
}

impl Default for LaneCostModel {
    fn default() -> Self {
        Self {
            gce_raw_nanousd_per_vm_sec: GCE_RAW_PER_VM_SEC_NANOUSD,
            shc_raw_nanousd_per_vm_sec: SHC_RAW_PER_VM_SEC_NANOUSD,
            session_vm_seconds: DEFAULT_SESSION_VM_SECONDS,
            material_margin_bps: PLACEMENT_COST_MATERIAL_MARGIN_BPS,
        }
    }
}

impl LaneCostModel {
    /// GCE cost-plus-10% estimate, micro-USD per VM-second.
    pub fn gce_micro_usd_per_vm_sec(&self) -> u64 {
        cost_plus_10pct_micro_usd_per_vm_sec(self.gce_raw_nanousd_per_vm_sec)
    }

    /// SHC cost-plus-10% estimate, micro-USD per VM-second.
    pub fn shc_micro_usd_per_vm_sec(&self) -> u64 {
        cost_plus_10pct_micro_usd_per_vm_sec(self.shc_raw_nanousd_per_vm_sec)
    }

    /// True when SHC is materially cheaper than GCE: SHC cost-plus-10% must be
    /// below GCE cost-plus-10% by at least `material_margin_bps`. GCE wins ties
    /// and near-ties by construction (owner direction).
    pub fn shc_materially_cheaper_than_gce(&self) -> bool {
        let gce = self.gce_micro_usd_per_vm_sec();
        let shc = self.shc_micro_usd_per_vm_sec();
        // Threshold: shc < gce * (10000 - margin) / 10000.
        let threshold =
            gce.saturating_mul(10_000u64.saturating_sub(self.material_margin_bps)) / 10_000;
        shc < threshold
    }

    /// Build the refs-only cost basis for a chosen lane.
    pub fn cost_basis(&self, chosen: ProviderLane) -> PlacementCostBasis {
        let gce = self.gce_micro_usd_per_vm_sec();
        let shc = self.shc_micro_usd_per_vm_sec();
        let chosen_rate = match chosen {
            ProviderLane::Shc => shc,
            _ => gce,
        };
        PlacementCostBasis {
            basis: "cost_plus_10pct_lane_model".to_string(),
            chosen_micro_usd_per_vm_sec: chosen_rate,
            gce_micro_usd_per_vm_sec: gce,
            shc_micro_usd_per_vm_sec: shc,
            modeled_session_vm_seconds: self.session_vm_seconds,
            chosen_session_micro_usd: chosen_rate.saturating_mul(self.session_vm_seconds),
        }
    }
}

/// Apply the cost-plus-10% markup to a raw nano-USD/VM-sec rate and convert to
/// micro-USD/VM-sec: `floor(raw_nanousd × 1.10 / 1000)`. Centralizes the markup
/// from `openagents.compute_quota_routing.v1`.
fn cost_plus_10pct_micro_usd_per_vm_sec(raw_nanousd_per_vm_sec: u64) -> u64 {
    // raw_nanousd × 1.10 = raw_nanousd × 11 / 10, then nano→micro is /1000.
    raw_nanousd_per_vm_sec.saturating_mul(11) / 10 / 1_000
}

impl PlacementAssignment {
    pub fn validate_contract(&self, now_ms: u128) -> Result<(), String> {
        if self.contract_version != PLACEMENT_ASSIGNMENT_VERSION {
            return Err(format!(
                "unexpected placement assignment contract version '{}'",
                self.contract_version
            ));
        }
        for (field, value) in [
            ("run_id", self.run_id.as_str()),
            ("owner_ref", self.owner_ref.as_str()),
            ("provider_account_ref", self.provider_account_ref.as_str()),
            ("auth_grant_ref", self.auth_grant_ref.as_str()),
            ("goal", self.goal.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{field} must not be empty"));
            }
            if contains_secret_material(value) {
                return Err(format!("{field} contains forbidden secret material"));
            }
        }
        for (field, value) in [
            ("repository", self.repository.as_deref()),
            ("sandbox_mode", self.sandbox_mode.as_deref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() || contains_secret_material(value) {
                    return Err(format!("{field} is invalid"));
                }
            }
        }
        if self.wallet_authority {
            return Err("placement assignments must not request wallet authority".to_string());
        }
        if self.created_at_ms > now_ms + 1000 * 60 * 5 {
            return Err("placement assignment is from the future".to_string());
        }
        Ok(())
    }

    /// Resolve the lane-agnostic assignment to a concrete runner binding using
    /// the policy-driven path (no cost comparison). Preserved for callers and
    /// tests that want the pure policy behavior; equivalent to
    /// [`resolve_runner_binding_cost_aware`] with `cost_driven_enabled = false`.
    ///
    /// Policy (owner direction 2026-06-14): Google GCE is primary, SHC is the
    /// secondary fallback. `Auto` and `CloudGcp` bind to GCE when GCE capacity
    /// is reachable; otherwise they fall back to SHC. `CloudShc` pins SHC.
    /// `Local` is not a cloud-placeable lane here.
    pub fn resolve_runner_binding(
        &self,
        gce_available: bool,
        shc_runner_id: &str,
        caps: ComputeQuotaCaps,
    ) -> Result<RunnerBinding, String> {
        self.resolve_runner_binding_cost_aware(
            gce_available,
            shc_runner_id,
            caps,
            false,
            false,
            LaneCostModel::default(),
        )
    }

    /// Resolve the lane-agnostic assignment to a concrete runner binding,
    /// optionally using the CND-042 cost-driven comparison.
    ///
    /// Cost-driven selection only applies to an `Auto` (non-caller-pinned)
    /// assignment when BOTH lanes are eligible (GCE available). In that case:
    ///
    /// - GCE wins ties and near-ties (owner direction: Google preferred).
    /// - SHC is chosen only when it is BOTH materially cheaper than GCE
    ///   (`LaneCostModel::shc_materially_cheaper_than_gce`) AND the pilot
    ///   recommendation is "expand" (`report_recommends_expand = true`).
    /// - The chosen lane records `cost_driven = true` and a refs-only
    ///   `cost_basis`.
    ///
    /// When `cost_driven_enabled` is false, or the lane is pinned, or GCE is
    /// unavailable (no choice to make), this is identical to the policy-driven
    /// path with `cost_driven = false` and no `cost_basis`.
    pub fn resolve_runner_binding_cost_aware(
        &self,
        gce_available: bool,
        shc_runner_id: &str,
        caps: ComputeQuotaCaps,
        cost_driven_enabled: bool,
        report_recommends_expand: bool,
        cost_model: LaneCostModel,
    ) -> Result<RunnerBinding, String> {
        // danger_full_access is the explicit default inside the no-wallet VM
        // boundary (CND-041/CND-055, cloud#88), not an implicit fallback.
        let sandbox_mode = self
            .sandbox_mode
            .clone()
            .unwrap_or_else(|| "danger_full_access".to_string());

        // (runner_id, provider_lane, capacity_class_id, reason, cost_basis)
        let (runner_id, provider_lane, capacity_class_id, reason, cost_basis) = match self.lane {
            ComputeLane::Local => {
                return Err(
                    "local lane is resolved by the caller's own Pylon, not by cloud placement"
                        .to_string(),
                );
            }
            ComputeLane::CloudShc => (
                shc_runner_id.to_string(),
                ProviderLane::Shc,
                None,
                PlacementReason::LanePinned,
                None,
            ),
            ComputeLane::CloudGcp => {
                if gce_available {
                    (
                        gce_runner_id(&self.run_id),
                        ProviderLane::Gcp,
                        Some(GCE_EPHEMERAL_CAPACITY_CLASS_ID.to_string()),
                        PlacementReason::LanePinned,
                        None,
                    )
                } else {
                    (
                        shc_runner_id.to_string(),
                        ProviderLane::Shc,
                        None,
                        PlacementReason::GceUnavailableShcFallback,
                        None,
                    )
                }
            }
            ComputeLane::Auto => {
                if !gce_available {
                    // No choice to make; SHC is the only eligible lane.
                    (
                        shc_runner_id.to_string(),
                        ProviderLane::Shc,
                        None,
                        PlacementReason::GceUnavailableShcFallback,
                        None,
                    )
                } else if cost_driven_enabled {
                    // Both lanes eligible: compare on cost-plus-10%.
                    // SHC wins only when materially cheaper AND the report says
                    // "expand". Otherwise GCE wins (ties/near-ties/owner pref).
                    let choose_shc =
                        report_recommends_expand && cost_model.shc_materially_cheaper_than_gce();
                    if choose_shc {
                        (
                            shc_runner_id.to_string(),
                            ProviderLane::Shc,
                            None,
                            PlacementReason::CostDriven,
                            Some(cost_model.cost_basis(ProviderLane::Shc)),
                        )
                    } else {
                        (
                            gce_runner_id(&self.run_id),
                            ProviderLane::Gcp,
                            Some(GCE_EPHEMERAL_CAPACITY_CLASS_ID.to_string()),
                            PlacementReason::CostDriven,
                            Some(cost_model.cost_basis(ProviderLane::Gcp)),
                        )
                    }
                } else {
                    // Policy-driven default: GCE primary.
                    (
                        gce_runner_id(&self.run_id),
                        ProviderLane::Gcp,
                        Some(GCE_EPHEMERAL_CAPACITY_CLASS_ID.to_string()),
                        PlacementReason::PolicyDefaultGce,
                        None,
                    )
                }
            }
        };

        let resolved_lane = match provider_lane {
            ProviderLane::Gcp => ComputeLane::CloudGcp,
            ProviderLane::Shc => ComputeLane::CloudShc,
            _ => self.lane,
        };

        Ok(RunnerBinding {
            contract_version: PLACEMENT_ASSIGNMENT_VERSION.to_string(),
            run_id: self.run_id.clone(),
            external_run_id: format!("shc-codex:{runner_id}:{}", self.run_id),
            lane: resolved_lane,
            provider_lane,
            runner_id,
            capacity_class_id,
            sandbox_mode,
            reason,
            cost_driven: cost_basis.is_some(),
            cost_basis,
            caps,
        })
    }
}

/// Derive a stable ephemeral GCE runner id for a run. The concrete instance is
/// provisioned per-session by the GCE capacity class; the runner id here is a
/// reconciliation-safe label, not a raw instance name or self-link.
fn gce_runner_id(run_id: &str) -> String {
    let suffix: String = run_id
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-' || *ch == '_')
        .take(48)
        .collect();
    if suffix.is_empty() {
        "oa-gce-ephemeral".to_string()
    } else {
        format!("oa-gce-ephemeral-{suffix}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTRIBUTOR: &str =
        include_str!("../../../fixtures/cloud/cloud_node_v1/contributor-pylon.json");
    const MANAGED: &str = include_str!("../../../fixtures/cloud/cloud_node_v1/managed-oa-node.json");
    const DEGRADED: &str = include_str!("../../../fixtures/cloud/cloud_node_v1/degraded-node.json");
    const PRIVATE_WORKROOM: &str =
        include_str!("../../../fixtures/cloud/workroom_v1/private-workroom.json");
    const CAPABILITY_WORKROOM: &str =
        include_str!("../../../fixtures/cloud/workroom_v1/capability-attached-workroom.json");
    const PREVIEW_WORKROOM: &str =
        include_str!("../../../fixtures/cloud/workroom_v1/preview-exposed-workroom.json");
    const FORGE_WORKROOM_ASSIGNMENT: &str =
        include_str!("../../../fixtures/cloud/forge_assignment_v1/workroom-assignment.json");
    const FORGE_SANDBOX_ASSIGNMENT: &str =
        include_str!("../../../fixtures/cloud/forge_assignment_v1/sandbox-worker-assignment.json");
    const FORGE_LABOR_ASSIGNMENT: &str =
        include_str!("../../../fixtures/cloud/forge_assignment_v1/open-ended-labor-assignment.json");
    const PSIONIC_MIXED_WORKERS: &str =
        include_str!("../../../fixtures/cloud/psionic_worker_attachment_v1/mixed-readiness.json");
    const PROBE_ATTACHMENT: &str =
        include_str!("../../../fixtures/cloud/probe_worker_attachment_v1/workroom-probe.json");
    const TRAINING_TERMINAL_BENCH: &str =
        include_str!("../../../fixtures/cloud/training_run_assignment_v1/terminal-bench-retained.json");
    const ARTANIS_BOOTSTRAP: &str = include_str!(
        "../../../fixtures/cloud/artanis_bootstrap_assignment_v1/pylon-launch-bootstrap.json"
    );

    #[test]
    fn fixtures_parse_and_validate() {
        for fixture in [CONTRIBUTOR, MANAGED, DEGRADED] {
            let snapshot: CloudNodeSnapshot = serde_json::from_str(fixture)
                .unwrap_or_else(|error| panic!("fixture should parse: {error}"));
            snapshot
                .validate_contract()
                .unwrap_or_else(|error| panic!("fixture should validate: {error}"));
        }
    }

    #[test]
    fn managed_scaffold_uses_no_wallet_policy() {
        let snapshot = CloudNodeSnapshot::managed_scaffold();
        assert_eq!(
            snapshot.policy.settlement_policy,
            SettlementPolicy::NoWallet
        );
        snapshot
            .validate_contract()
            .unwrap_or_else(|error| panic!("scaffold should validate: {error}"));
    }

    #[test]
    fn workroom_fixtures_parse_and_validate() {
        for fixture in [PRIVATE_WORKROOM, CAPABILITY_WORKROOM, PREVIEW_WORKROOM] {
            let snapshot: WorkroomSnapshot = serde_json::from_str(fixture)
                .unwrap_or_else(|error| panic!("workroom fixture should parse: {error}"));
            snapshot
                .validate_contract()
                .unwrap_or_else(|error| panic!("workroom fixture should validate: {error}"));
        }
    }

    #[test]
    fn workroom_scaffold_is_private_and_has_no_wallet_authority() {
        let snapshot = WorkroomSnapshot::scaffold();
        assert_eq!(snapshot.ingress.visibility, IngressVisibility::Private);
        assert!(!snapshot.runtime.wallet_authority);
        snapshot
            .validate_contract()
            .unwrap_or_else(|error| panic!("workroom scaffold should validate: {error}"));
    }

    #[test]
    fn forge_assignment_fixtures_parse_and_validate() {
        for fixture in [
            FORGE_WORKROOM_ASSIGNMENT,
            FORGE_SANDBOX_ASSIGNMENT,
            FORGE_LABOR_ASSIGNMENT,
        ] {
            let assignment: ForgeAssignment = serde_json::from_str(fixture)
                .unwrap_or_else(|error| panic!("assignment fixture should parse: {error}"));
            assignment
                .validate_contract()
                .unwrap_or_else(|error| panic!("assignment fixture should validate: {error}"));
        }
    }

    #[test]
    fn forge_receipt_validation_requires_digests() {
        let receipt = ForgeAssignmentReceipt {
            schema_version: FORGE_ASSIGNMENT_RECEIPT_VERSION.to_string(),
            receipt_id: "receipt.test".to_string(),
            assignment_id: "assignment.test".to_string(),
            node_id: "node.test".to_string(),
            decision: ForgeAssignmentDecision::Refused,
            reason: "test_refusal".to_string(),
            assignment_digest: "sha256:assignment".to_string(),
            receipt_digest: "sha256:receipt".to_string(),
            emitted_at_ms: 1,
        };
        receipt
            .validate_contract()
            .unwrap_or_else(|error| panic!("receipt should validate: {error}"));
    }

    #[test]
    fn codex_auth_grant_requires_scoped_secret_ref_and_ttl() {
        let grant = CodexAuthGrant {
            contract_version: CODEX_AUTH_GRANT_VERSION.to_string(),
            workroom_id: "workroom.codex.test".to_string(),
            user_ref: "user.test".to_string(),
            organization_ref: Some("org.test".to_string()),
            project_ref: Some("project.test".to_string()),
            provider_account_ref: "provider-account_codex_test".to_string(),
            grant_ref: "codex-auth-grant_test".to_string(),
            provider_secret_ref: "secret://codex/account/test".to_string(),
            requested_mode: CodexRequestedMode::Exec,
            issued_at_ms: 100,
            expires_at_ms: 100 + 1000 * 60 * 30,
            audit_context: "vortex.issue.84".to_string(),
        };
        grant
            .validate_for_session(101)
            .unwrap_or_else(|error| panic!("grant should validate: {error}"));

        let mut raw_secret = grant.clone();
        raw_secret.provider_secret_ref = "sk-raw-openai-key".to_string();
        assert!(raw_secret.validate_for_session(101).is_err());

        let mut expired = grant;
        expired.expires_at_ms = 100;
        assert!(expired.validate_for_session(101).is_err());
    }

    #[test]
    fn codex_auth_receipts_are_redacted_digest_only() {
        let receipt = CodexAuthReceipt {
            schema_version: CODEX_AUTH_RECEIPT_VERSION.to_string(),
            receipt_id: "codex.auth.materialized.1".to_string(),
            workroom_id: "workroom.codex.test".to_string(),
            grant_ref: "codex-auth-grant_test".to_string(),
            provider_account_ref: "provider-account_codex_test".to_string(),
            event_kind: CodexAuthReceiptKind::GrantMaterialized,
            decision: CodexAuthDecision::Accepted,
            reason: "session_codex_home_created".to_string(),
            codex_home_digest: "sha256:codex-home".to_string(),
            receipt_digest: "sha256:receipt".to_string(),
            emitted_at_ms: 1,
        };
        receipt
            .validate_contract()
            .unwrap_or_else(|error| panic!("codex auth receipt should validate: {error}"));

        let mut leaked = receipt;
        leaked.reason = "access_token=raw".to_string();
        assert!(leaked.validate_contract().is_err());
    }

    #[test]
    fn codex_workroom_assignment_requires_no_wallet_and_bounded_artifacts() {
        let assignment = CodexWorkroomAssignment {
            contract_version: CODEX_WORKROOM_ASSIGNMENT_VERSION.to_string(),
            assignment_id: "assignment.codex.test".to_string(),
            workroom_id: "workroom.codex.test".to_string(),
            target_node_id: "oa-gcp-shc-katy-01".to_string(),
            user_ref: "user.test".to_string(),
            organization_ref: Some("org.test".to_string()),
            project_ref: Some("project.test".to_string()),
            provider_account_ref: "provider-account_codex_test".to_string(),
            auth_grant_ref: "codex-auth-grant_test".to_string(),
            repo_ref: "OpenAgentsInc/cloud".to_string(),
            prompt: "Create the required summary artifact.".to_string(),
            required_artifacts: vec!["summary".to_string()],
            sandbox: CodexSandboxMode::WorkspaceWrite,
            timeout_ms: Some(1000 * 60),
            wallet_authority: false,
            created_at_ms: 100,
            audit_context: "vortex.issue.85".to_string(),
        };
        assignment
            .validate_contract(101)
            .unwrap_or_else(|error| panic!("assignment should validate: {error}"));

        let mut externally_sandboxed = assignment.clone();
        externally_sandboxed.sandbox = CodexSandboxMode::DangerFullAccess;
        externally_sandboxed
            .validate_contract(101)
            .unwrap_or_else(|error| {
                panic!("externally sandboxed assignment should validate: {error}")
            });

        let mut wallet = assignment.clone();
        wallet.wallet_authority = true;
        assert!(wallet.validate_contract(101).is_err());

        let mut path_artifact = assignment.clone();
        path_artifact.required_artifacts = vec!["../summary".to_string()];
        assert!(path_artifact.validate_contract(101).is_err());
    }

    #[test]
    fn codex_workroom_events_are_redacted_digest_only() {
        let event = CodexWorkroomEvent {
            schema_version: CODEX_WORKROOM_EVENT_VERSION.to_string(),
            event_id: "codex.workroom.event.1".to_string(),
            assignment_id: "assignment.codex.test".to_string(),
            workroom_id: "workroom.codex.test".to_string(),
            sequence: 1,
            event_kind: CodexWorkroomEventKind::Artifact,
            decision: CodexWorkroomDecision::Accepted,
            message: "artifact captured".to_string(),
            artifact_ref: Some("sha256:artifact".to_string()),
            receipt_ref: Some("sha256:receipt".to_string()),
            event_digest: "sha256:event".to_string(),
            emitted_at_ms: 101,
        };
        event
            .validate_contract()
            .unwrap_or_else(|error| panic!("codex workroom event should validate: {error}"));

        let mut leaked = event.clone();
        leaked.message = "refresh_token=raw".to_string();
        assert!(leaked.validate_contract().is_err());
    }

    #[test]
    fn resource_usage_receipts_require_explicit_unavailable_token_reason() {
        let digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let mut receipt = ResourceUsageReceipt {
            schema_version: RESOURCE_USAGE_RECEIPT_VERSION.to_string(),
            receipt_id: "resource.usage.assignment.test.1".to_string(),
            run_ref: "assignment.test".to_string(),
            workroom_id: "workroom.test".to_string(),
            node_ref: "oa-shc-katy-01".to_string(),
            provider_lane: ProviderLane::Shc,
            host: ResourceHostSnapshot {
                os: "ubuntu_24.04".to_string(),
                arch: "x86_64".to_string(),
                cpu: "16_logical_cpu".to_string(),
                logical_cpu_count: 16,
                physical_cpu_count: Some(8),
                memory_total_bytes: Some(64 * 1024 * 1024 * 1024),
                memory_available_bytes: Some(32 * 1024 * 1024 * 1024),
                disk_total_bytes: Some(256 * 1024 * 1024 * 1024),
                disk_available_bytes: Some(128 * 1024 * 1024 * 1024),
                accelerator_inventory: Vec::new(),
                virtualization: VirtualizationFacts {
                    kvm_present: true,
                    firecracker_candidate: true,
                    container_runtime: Some("docker".to_string()),
                    cgroup_mode: Some("v2".to_string()),
                },
            },
            run: RunResourceUsage {
                sandbox: "danger-full-access".to_string(),
                image_or_profile_digest: digest.to_string(),
                workspace_digest: digest.to_string(),
                wall_time_ms: Some(1250),
                exit_code: Some(0),
                timed_out: false,
                workspace_bytes: Some(1024),
                artifact_bytes: Some(512),
                log_bytes: Some(256),
            },
            model_usage: vec![ModelUsageRecord {
                provider: "openai".to_string(),
                backend: "codex".to_string(),
                model: "codex_subscription".to_string(),
                mode: "codex_exec".to_string(),
                account_ref: Some("provider-account_test".to_string()),
                input_tokens: None,
                cached_input_tokens: None,
                output_tokens: None,
                reasoning_tokens: None,
                total_tokens: None,
                count_source: TokenCountSource::Unavailable,
                cost_microusd: None,
                billing_basis: "chatgpt_subscription".to_string(),
                unavailable_reason: Some("subscription_backed_codex_no_token_counts".to_string()),
            }],
            compute_usage: None,
            receipt_digest: digest.to_string(),
            emitted_at_ms: 1,
        };
        receipt
            .validate_contract()
            .unwrap_or_else(|error| panic!("resource usage receipt should validate: {error}"));

        receipt.model_usage[0].unavailable_reason = None;
        assert!(receipt.validate_contract().is_err());
    }

    #[test]
    fn training_run_assignment_fixture_parses_and_validates() {
        let assignment: TrainingRunAssignment = serde_json::from_str(TRAINING_TERMINAL_BENCH)
            .unwrap_or_else(|error| panic!("training fixture should parse: {error}"));
        assignment
            .validate_contract(101)
            .unwrap_or_else(|error| panic!("training fixture should validate: {error}"));

        let mut shell_injection = assignment.clone();
        shell_injection.dataset.task_ref = "terminal-bench/db-wal-recovery;rm-rf".to_string();
        assert!(shell_injection.validate_contract(101).is_err());

        let mut local_only = assignment.clone();
        local_only.artifacts.retention_mode = TrainingRetentionMode::LocalOnly;
        local_only
            .validate_contract(101)
            .unwrap_or_else(|error| panic!("local-only retention should validate: {error}"));
    }

    #[test]
    fn artanis_bootstrap_assignment_fixture_parses_and_validates() {
        let assignment: ArtanisBootstrapAssignment = serde_json::from_str(ARTANIS_BOOTSTRAP)
            .unwrap_or_else(|error| panic!("Artanis fixture should parse: {error}"));
        assignment
            .validate_contract(101)
            .unwrap_or_else(|error| panic!("Artanis fixture should validate: {error}"));
        assert!(!assignment.wallet_authority);
        assert!(assignment
            .pylon_capability_labels
            .iter()
            .any(|label| label == "qwen_legal_adapter_training"));
        let settlement_intent = assignment
            .settlement_intent
            .as_ref()
            .expect("Artanis fixture should carry settlement intent ids");
        assert_eq!(
            settlement_intent.artanis_run_id,
            assignment.bootstrap_run_id
        );
        assert_eq!(
            settlement_intent.artanis_assignment_id,
            "artanis.assignment.pylon-launch.001"
        );

        let mut leaked = assignment.clone();
        leaked.objective_summary = "use access_token=raw".to_string();
        assert!(leaked.validate_contract(101).is_err());

        let mut mismatched_settlement = assignment.clone();
        mismatched_settlement
            .settlement_intent
            .as_mut()
            .expect("settlement intent")
            .artanis_run_id = "artanis.bootstrap.other.001".to_string();
        assert!(mismatched_settlement.validate_contract(101).is_err());

        let mut wallet = assignment;
        wallet.wallet_authority = true;
        assert!(wallet.validate_contract(101).is_err());
    }

    #[test]
    fn psionic_worker_attachment_fixture_parses_and_validates() {
        let attachment: PsionicWorkerAttachment = serde_json::from_str(PSIONIC_MIXED_WORKERS)
            .unwrap_or_else(|error| panic!("psionic worker fixture should parse: {error}"));
        attachment
            .validate_contract()
            .unwrap_or_else(|error| panic!("psionic worker fixture should validate: {error}"));
    }

    #[test]
    fn psionic_execution_receipt_validation_requires_evidence_digest() {
        let receipt = PsionicExecutionReceipt {
            schema_version: PSIONIC_EXECUTION_RECEIPT_VERSION.to_string(),
            receipt_id: "psionic.receipt.test".to_string(),
            assignment_id: "assignment.test".to_string(),
            product_id: "psionic.managed.inference".to_string(),
            worker_id: "psionic.worker.test".to_string(),
            status: PsionicExecutionStatus::Succeeded,
            profile_digest: None,
            psionic_evidence_digest: "sha256:psionic-evidence".to_string(),
            receipt_digest: "sha256:psionic-receipt".to_string(),
            emitted_at_ms: 1,
        };
        receipt
            .validate_contract()
            .unwrap_or_else(|error| panic!("psionic receipt should validate: {error}"));
    }

    #[test]
    fn probe_worker_attachment_fixture_parses_and_validates() {
        let attachment: ProbeWorkerAttachment = serde_json::from_str(PROBE_ATTACHMENT)
            .unwrap_or_else(|error| panic!("probe worker fixture should parse: {error}"));
        attachment
            .validate_contract()
            .unwrap_or_else(|error| panic!("probe worker fixture should validate: {error}"));
        assert!(!attachment.raw_secret_access);
    }

    #[test]
    fn probe_closeout_receipt_validation_requires_artifacts() {
        let receipt = ProbeCloseoutReceipt {
            schema_version: PROBE_CLOSEOUT_RECEIPT_VERSION.to_string(),
            receipt_id: "probe.closeout.test".to_string(),
            workroom_id: "workroom.test".to_string(),
            worker_id: "probe.worker.test".to_string(),
            status: ProbeCloseoutStatus::Succeeded,
            artifact_refs: vec!["artifact://probe/transcript".to_string()],
            receipt_digest: "sha256:probe-closeout".to_string(),
            emitted_at_ms: 1,
        };
        receipt
            .validate_contract()
            .unwrap_or_else(|error| panic!("probe closeout receipt should validate: {error}"));
    }

    fn placement_fixture(lane: ComputeLane) -> PlacementAssignment {
        PlacementAssignment {
            contract_version: PLACEMENT_ASSIGNMENT_VERSION.to_string(),
            run_id: "agent_run_42".to_string(),
            owner_ref: "owner://sha256/example".to_string(),
            provider_account_ref: "provider-account_abc123".to_string(),
            auth_grant_ref: "codex-auth-grant_123".to_string(),
            goal: "Create the requested artifact.".to_string(),
            lane,
            repository: Some("OpenAgentsInc/openagents".to_string()),
            sandbox_mode: None,
            wallet_authority: false,
            created_at_ms: 1,
            work_context_b64: None,
            work_context_ref: None,
        }
    }

    #[test]
    fn placement_defaults_to_gce_primary_when_available() {
        let assignment = placement_fixture(ComputeLane::Auto);
        assignment.validate_contract(1).unwrap();
        let binding = assignment
            .resolve_runner_binding(true, SHC_FALLBACK_RUNNER_ID, ComputeQuotaCaps::default())
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Gcp);
        assert_eq!(binding.lane, ComputeLane::CloudGcp);
        assert_eq!(binding.reason, PlacementReason::PolicyDefaultGce);
        assert_eq!(
            binding.capacity_class_id.as_deref(),
            Some(GCE_EPHEMERAL_CAPACITY_CLASS_ID)
        );
        // Default sandbox inside the no-wallet VM boundary (cloud#88).
        assert_eq!(binding.sandbox_mode, "danger_full_access");
        // Policy-driven until CND-042 lands.
        assert!(!binding.cost_driven);
        assert_eq!(binding.caps.session_ttl_ms, DEFAULT_SESSION_TTL_MS);
        assert_eq!(binding.caps.idle_timeout_ms, DEFAULT_IDLE_TIMEOUT_MS);
    }

    #[test]
    fn placement_auto_falls_back_to_shc_when_gce_unavailable() {
        let assignment = placement_fixture(ComputeLane::Auto);
        let binding = assignment
            .resolve_runner_binding(false, SHC_FALLBACK_RUNNER_ID, ComputeQuotaCaps::default())
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Shc);
        assert_eq!(binding.lane, ComputeLane::CloudShc);
        assert_eq!(binding.runner_id, SHC_FALLBACK_RUNNER_ID);
        assert_eq!(binding.reason, PlacementReason::GceUnavailableShcFallback);
        assert_eq!(binding.capacity_class_id, None);
    }

    #[test]
    fn placement_pins_shc_when_requested() {
        let assignment = placement_fixture(ComputeLane::CloudShc);
        let binding = assignment
            .resolve_runner_binding(true, SHC_FALLBACK_RUNNER_ID, ComputeQuotaCaps::default())
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Shc);
        assert_eq!(binding.reason, PlacementReason::LanePinned);
    }

    #[test]
    fn placement_cloud_gcp_pin_falls_back_to_shc_when_unavailable() {
        let assignment = placement_fixture(ComputeLane::CloudGcp);
        let binding = assignment
            .resolve_runner_binding(false, SHC_FALLBACK_RUNNER_ID, ComputeQuotaCaps::default())
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Shc);
        assert_eq!(binding.reason, PlacementReason::GceUnavailableShcFallback);
    }

    #[test]
    fn placement_local_lane_is_not_cloud_placeable() {
        let assignment = placement_fixture(ComputeLane::Local);
        assert!(assignment
            .resolve_runner_binding(true, SHC_FALLBACK_RUNNER_ID, ComputeQuotaCaps::default())
            .is_err());
    }

    #[test]
    fn placement_rejects_wallet_authority_and_future_timestamp() {
        let mut assignment = placement_fixture(ComputeLane::Auto);
        assignment.wallet_authority = true;
        assert!(assignment.validate_contract(1).is_err());

        let mut future = placement_fixture(ComputeLane::Auto);
        future.created_at_ms = 1_000_000_000;
        assert!(future.validate_contract(1).is_err());
    }

    #[test]
    fn lane_cost_model_gce_is_cheaper_than_shc_and_not_materially_beaten() {
        // From the CND-042 report: GCE ~5 micro-USD/VM-sec, SHC ~34 micro-USD.
        let model = LaneCostModel::default();
        let gce = model.gce_micro_usd_per_vm_sec();
        let shc = model.shc_micro_usd_per_vm_sec();
        // cost_plus_10pct(4653 nano × 1.1 / 1000) = floor(5118/1000) = 5.
        assert_eq!(gce, 5);
        // cost_plus_10pct(31710 nano × 1.1 / 1000) = floor(34881/1000) = 34.
        assert_eq!(shc, 34);
        // SHC is more expensive, so it is not materially cheaper than GCE.
        assert!(!model.shc_materially_cheaper_than_gce());
    }

    #[test]
    fn compute_usage_gce_catalog_uses_cost_plus_10pct_catalog_basis() {
        let model = LaneCostModel::default();
        // 120 measured VM-seconds × 5 micro-USD/VM-sec (cost-plus-10% over the
        // list-price catalog rate) = 600 micro-USD.
        let usage =
            ComputeUsage::gce_catalog_from_vm_seconds(GCE_EPHEMERAL_CAPACITY_CLASS_ID, 120, &model);
        assert_eq!(usage.vm_seconds, 120);
        assert_eq!(usage.metering_source, MeteringSource::NodeMeasured);
        assert_eq!(
            usage.cost_input_basis,
            CostInputBasis::CostPlus10pctGcpCatalog
        );
        assert_eq!(
            usage.cost_input_basis.as_str(),
            "cost_plus_10pct_gcp_catalog"
        );
        assert_eq!(
            usage.cost_input_microusd,
            Some(120u128 * model.gce_micro_usd_per_vm_sec() as u128)
        );
        usage.validate_contract().unwrap();
    }

    #[test]
    fn compute_usage_unavailable_must_have_null_cost() {
        let bad = ComputeUsage {
            compute_class: GCE_EPHEMERAL_CAPACITY_CLASS_ID.to_string(),
            vm_seconds: 0,
            metering_source: MeteringSource::NodeMeasured,
            cost_input_microusd: Some(1),
            cost_input_basis: CostInputBasis::Unavailable,
        };
        assert!(bad.validate_contract().is_err());

        let ok = ComputeUsage {
            cost_input_microusd: None,
            ..bad
        };
        ok.validate_contract().unwrap();
    }

    #[test]
    fn placement_cost_driven_cheaper_eligible_lane_wins() {
        // Construct a model where SHC is clearly cheaper than GCE so the
        // cheaper eligible lane wins under cost-driven placement + expand.
        let model = LaneCostModel {
            gce_raw_nanousd_per_vm_sec: 45_662, // expensive GCE
            shc_raw_nanousd_per_vm_sec: 4_653,  // cheap SHC
            ..LaneCostModel::default()
        };
        assert!(model.shc_materially_cheaper_than_gce());
        let assignment = placement_fixture(ComputeLane::Auto);
        let binding = assignment
            .resolve_runner_binding_cost_aware(
                true,
                SHC_FALLBACK_RUNNER_ID,
                ComputeQuotaCaps::default(),
                true,
                true, // report recommends expand
                model,
            )
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Shc);
        assert_eq!(binding.lane, ComputeLane::CloudShc);
        assert_eq!(binding.reason, PlacementReason::CostDriven);
        assert!(binding.cost_driven);
        let basis = binding.cost_basis.expect("cost basis on cost-driven SHC");
        assert_eq!(
            basis.chosen_micro_usd_per_vm_sec,
            model.shc_micro_usd_per_vm_sec()
        );
    }

    #[test]
    fn placement_cost_driven_google_wins_tie_and_when_competitive() {
        // Default model: GCE is cheaper than SHC. Even with expand recommended,
        // GCE wins because SHC is not materially cheaper (owner tiebreak).
        let model = LaneCostModel::default();
        let assignment = placement_fixture(ComputeLane::Auto);
        let binding = assignment
            .resolve_runner_binding_cost_aware(
                true,
                SHC_FALLBACK_RUNNER_ID,
                ComputeQuotaCaps::default(),
                true,
                true, // expand recommended, but SHC not materially cheaper
                model,
            )
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Gcp);
        assert_eq!(binding.lane, ComputeLane::CloudGcp);
        assert_eq!(binding.reason, PlacementReason::CostDriven);
        assert!(binding.cost_driven);
        let basis = binding.cost_basis.expect("cost basis on cost-driven GCE");
        assert_eq!(
            basis.chosen_micro_usd_per_vm_sec,
            model.gce_micro_usd_per_vm_sec()
        );
    }

    #[test]
    fn placement_cost_driven_shc_only_when_expand_recommended() {
        // SHC materially cheaper, but the report does NOT recommend expand:
        // GCE must still win (HOLD/STOP never promotes SHC on cost alone).
        let model = LaneCostModel {
            gce_raw_nanousd_per_vm_sec: 45_662,
            shc_raw_nanousd_per_vm_sec: 4_653,
            ..LaneCostModel::default()
        };
        let assignment = placement_fixture(ComputeLane::Auto);
        let binding = assignment
            .resolve_runner_binding_cost_aware(
                true,
                SHC_FALLBACK_RUNNER_ID,
                ComputeQuotaCaps::default(),
                true,
                false, // report does NOT recommend expand
                model,
            )
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Gcp);
        assert_eq!(binding.reason, PlacementReason::CostDriven);
        assert!(binding.cost_driven);
    }

    #[test]
    fn placement_cost_driven_pinned_lane_still_pinned() {
        // A caller pin is never overridden by cost comparison.
        let model = LaneCostModel {
            gce_raw_nanousd_per_vm_sec: 45_662,
            shc_raw_nanousd_per_vm_sec: 4_653,
            ..LaneCostModel::default()
        };
        let assignment = placement_fixture(ComputeLane::CloudGcp);
        let binding = assignment
            .resolve_runner_binding_cost_aware(
                true,
                SHC_FALLBACK_RUNNER_ID,
                ComputeQuotaCaps::default(),
                true,
                true,
                model,
            )
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Gcp);
        assert_eq!(binding.reason, PlacementReason::LanePinned);
        assert!(!binding.cost_driven);
        assert!(binding.cost_basis.is_none());
    }

    #[test]
    fn placement_cost_driven_gce_unavailable_is_fallback_not_cost_driven() {
        // GCE unavailable: no choice, SHC fallback, not a cost-driven decision.
        let assignment = placement_fixture(ComputeLane::Auto);
        let binding = assignment
            .resolve_runner_binding_cost_aware(
                false,
                SHC_FALLBACK_RUNNER_ID,
                ComputeQuotaCaps::default(),
                true,
                true,
                LaneCostModel::default(),
            )
            .unwrap();
        assert_eq!(binding.provider_lane, ProviderLane::Shc);
        assert_eq!(binding.reason, PlacementReason::GceUnavailableShcFallback);
        assert!(!binding.cost_driven);
        assert!(binding.cost_basis.is_none());
    }

    #[test]
    fn placement_default_lane_is_auto() {
        let json = r#"{
            "contract_version": "openagents.codex_placement_assignment.v1",
            "run_id": "r1",
            "owner_ref": "owner://sha256/x",
            "provider_account_ref": "provider-account_x",
            "auth_grant_ref": "codex-auth-grant_x",
            "goal": "do work",
            "wallet_authority": false,
            "created_at_ms": 1
        }"#;
        let assignment: PlacementAssignment = serde_json::from_str(json).unwrap();
        assert_eq!(assignment.lane, ComputeLane::Auto);
    }
}
