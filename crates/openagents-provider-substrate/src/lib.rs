//! Narrow shared provider substrate for reusable provider runtime semantics.
//!
//! This crate intentionally owns only product-agnostic provider domain logic:
//! backend health, launch product derivation, inventory controls/models, and
//! provider lifecycle derivation. App-specific UX, execution snapshots, and
//! orchestration stay in app crates.

mod admin;
mod payout_target;
mod sandbox;
mod sandbox_execution;

pub use admin::*;
pub use payout_target::*;
pub use sandbox::*;
pub use sandbox_execution::*;

use openagents_kernel_core::compute::{
    ComputeAdapterAggregationEligibility, ComputeAdapterContributionDisposition,
    ComputeAdapterContributionOutcome, ComputeAdapterTrainingWindow,
};
use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBackendKind {
    #[serde(alias = "gpt_oss", alias = "ollama")]
    GptOss,
    AppleFoundationModels,
    PsionicTrain,
}

impl ProviderBackendKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GptOss => "local Gemma runtime",
            Self::AppleFoundationModels => "Apple Foundation Models bridge",
            Self::PsionicTrain => "Psionic train contributor runtime",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMode {
    Offline,
    Connecting,
    Online,
    Degraded,
}

impl ProviderMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Connecting => "connecting",
            Self::Online => "online",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderIngressMode {
    Offline,
    Preview,
    Connecting,
    Online,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBlocker {
    IdentityMissing,
    WalletError,
    SkillTrustUnavailable,
    CreditLaneUnavailable,
    GptOssUnavailable,
    GptOssModelUnavailable,
    AppleFoundationModelsUnavailable,
    AppleFoundationModelsModelUnavailable,
}

impl ProviderBlocker {
    pub const fn code(self) -> &'static str {
        match self {
            Self::IdentityMissing => "IDENTITY_MISSING",
            Self::WalletError => "WALLET_ERROR",
            Self::SkillTrustUnavailable => "SKL_TRUST_UNAVAILABLE",
            Self::CreditLaneUnavailable => "AC_CREDIT_UNAVAILABLE",
            Self::GptOssUnavailable => "LOCAL_GEMMA_UNAVAILABLE",
            Self::GptOssModelUnavailable => "LOCAL_GEMMA_MODEL_UNAVAILABLE",
            Self::AppleFoundationModelsUnavailable => "APPLE_FM_UNAVAILABLE",
            Self::AppleFoundationModelsModelUnavailable => "APPLE_FM_MODEL_UNAVAILABLE",
        }
    }

    pub const fn detail(self) -> &'static str {
        match self {
            Self::IdentityMissing => "Nostr identity is not ready",
            Self::WalletError => "Spark wallet reports an error",
            Self::SkillTrustUnavailable => "SKL trust gate is not trusted",
            Self::CreditLaneUnavailable => "AC credit lane is not available",
            Self::GptOssUnavailable => "Local Gemma runtime is unavailable",
            Self::GptOssModelUnavailable => "No local Gemma serving model is ready",
            Self::AppleFoundationModelsUnavailable => {
                "Apple Foundation Models backend is unavailable"
            }
            Self::AppleFoundationModelsModelUnavailable => {
                "Apple Foundation Models is not ready to serve inference"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureClass {
    Relay,
    Execution,
    Payment,
    Reconciliation,
}

impl ProviderFailureClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Relay => "relay",
            Self::Execution => "execution",
            Self::Payment => "payment",
            Self::Reconciliation => "reconciliation",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderBackendHealth {
    pub reachable: bool,
    pub ready: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub availability_message: Option<String>,
    pub latency_ms_p50: Option<u64>,
}

impl ProviderBackendHealth {
    pub const fn is_ready(&self) -> bool {
        self.ready
    }

    pub fn has_authoritative_state(&self) -> bool {
        self.reachable
            || self.ready
            || self.configured_model.is_some()
            || self.ready_model.is_some()
            || !self.available_models.is_empty()
            || self.last_error.is_some()
            || self.last_action.is_some()
            || self.availability_message.is_some()
            || self.latency_ms_p50.is_some()
    }

    pub fn is_inert(&self) -> bool {
        !self.has_authoritative_state()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderDiagnosticSummary {
    pub diagnostic_id: String,
    pub model_id: String,
    pub runtime_backend: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub measured_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_total_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_ttft_s: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mean_decode_tok_s: Option<f64>,
    #[serde(default)]
    pub repeats: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostLoadAverageTelemetry {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostMemoryTelemetry {
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostSwapTelemetry {
    pub used_bytes: u64,
    pub free_bytes: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostGpuTelemetry {
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_total_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_free_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_total_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_free_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_draw_watts: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_limit_watts: Option<f64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostDiskTelemetry {
    pub mount_point: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub removable: bool,
    pub available_space_bytes: u64,
    pub total_space_bytes: u64,
    pub read_bytes_delta: u64,
    pub written_bytes_delta: u64,
    pub total_read_bytes: u64,
    pub total_written_bytes: u64,
    #[serde(default)]
    pub pylon_home_disk: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostNetworkInterfaceTelemetry {
    pub name: String,
    pub received_bytes_delta: u64,
    pub transmitted_bytes_delta: u64,
    pub total_received_bytes: u64,
    pub total_transmitted_bytes: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostThermalComponentTelemetry {
    pub label: String,
    pub temperature_celsius: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_celsius: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub critical_celsius: Option<f32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostPowerTelemetry {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draw_summary: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostTelemetrySnapshot {
    pub captured_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub physical_cpu_count: Option<u64>,
    #[serde(default)]
    pub logical_cpu_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_brand: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_frequency_mhz: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_usage_percent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_average: Option<ProviderHostLoadAverageTelemetry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<ProviderHostMemoryTelemetry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<ProviderHostSwapTelemetry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uptime_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpus: Vec<ProviderHostGpuTelemetry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disks: Vec<ProviderHostDiskTelemetry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub network_interfaces: Vec<ProviderHostNetworkInterfaceTelemetry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thermal_components: Vec<ProviderHostThermalComponentTelemetry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power: Option<ProviderHostPowerTelemetry>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderHostingTelemetrySnapshot {
    pub captured_at_unix_ms: u64,
    pub runtime: ProviderRuntimeStatusSnapshot,
    pub availability: ProviderAvailability,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inventory_rows: Vec<ProviderInventoryRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<ProviderHostTelemetrySnapshot>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAppleAdapterHostingEntry {
    pub adapter_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_model_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_format_version: Option<String>,
    #[serde(default)]
    pub draft_model_present: bool,
    #[serde(default)]
    pub compatible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility_reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatibility_message: Option<String>,
    #[serde(default)]
    pub attached_session_count: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAppleAdapterHostingAvailability {
    #[serde(default)]
    pub inventory_supported: bool,
    #[serde(default)]
    pub attach_supported: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapters: Vec<ProviderAppleAdapterHostingEntry>,
}

impl ProviderAppleAdapterHostingAvailability {
    pub fn has_authoritative_state(&self) -> bool {
        self.inventory_supported || self.attach_supported || !self.adapters.is_empty()
    }

    pub fn is_inert(&self) -> bool {
        !self.has_authoritative_state()
    }

    pub fn loaded_adapter_count(&self) -> usize {
        self.adapters.len()
    }

    pub fn compatible_adapter_count(&self) -> usize {
        self.adapters
            .iter()
            .filter(|entry| entry.compatible)
            .count()
    }

    pub fn compatible_package_digests(&self) -> Vec<String> {
        let mut digests = self
            .adapters
            .iter()
            .filter(|entry| entry.compatible)
            .filter_map(|entry| entry.package_digest.clone())
            .filter(|digest| !digest.trim().is_empty())
            .collect::<Vec<_>>();
        digests.sort();
        digests.dedup();
        digests
    }

    pub fn product_backend_ready(&self, backend: &ProviderBackendHealth) -> bool {
        backend.is_ready()
            && self.inventory_supported
            && self.attach_supported
            && self.compatible_adapter_count() > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAdapterTrainingExecutionBackend {
    AppleFoundationModels,
    OpenAdapterBackend,
}

impl ProviderAdapterTrainingExecutionBackend {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AppleFoundationModels => "apple_foundation_models",
            Self::OpenAdapterBackend => "open_adapter_backend",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAdapterTrainingSettlementTrigger {
    AcceptedContribution,
    AcceptedSealedWindow,
}

impl ProviderAdapterTrainingSettlementTrigger {
    pub const fn label(self) -> &'static str {
        match self {
            Self::AcceptedContribution => "accepted_contribution",
            Self::AcceptedSealedWindow => "accepted_sealed_window",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAdapterTrainingContributorAvailability {
    #[serde(default)]
    pub contributor_supported: bool,
    #[serde(default)]
    pub coordinator_match_supported: bool,
    #[serde(default)]
    pub authority_receipt_supported: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution_backends: Vec<ProviderAdapterTrainingExecutionBackend>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_families: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adapter_formats: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validator_policy_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoint_families: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub environment_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_memory_gb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_memory_gb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_trigger: Option<ProviderAdapterTrainingSettlementTrigger>,
}

impl ProviderAdapterTrainingContributorAvailability {
    pub fn has_authoritative_state(&self) -> bool {
        self.contributor_supported
            || self.coordinator_match_supported
            || self.authority_receipt_supported
            || !self.execution_backends.is_empty()
            || !self.adapter_families.is_empty()
            || !self.adapter_formats.is_empty()
            || !self.validator_policy_refs.is_empty()
            || !self.checkpoint_families.is_empty()
            || !self.environment_refs.is_empty()
            || self.minimum_memory_gb.is_some()
            || self.available_memory_gb.is_some()
            || self.settlement_trigger.is_some()
    }

    pub fn is_inert(&self) -> bool {
        !self.has_authoritative_state()
    }

    pub fn product_backend_ready(&self) -> bool {
        self.contributor_supported
            && self.coordinator_match_supported
            && self.authority_receipt_supported
            && !self.execution_backends.is_empty()
            && !self.adapter_families.is_empty()
            && !self.adapter_formats.is_empty()
            && !self.validator_policy_refs.is_empty()
            && self
                .available_memory_gb
                .zip(self.minimum_memory_gb)
                .is_none_or(|(available, minimum)| available >= minimum)
    }

    pub fn capability_summary(&self) -> String {
        let execution_backends = if self.execution_backends.is_empty() {
            "none".to_string()
        } else {
            self.execution_backends
                .iter()
                .map(|backend| backend.label())
                .collect::<Vec<_>>()
                .join(",")
        };
        let adapter_families = if self.adapter_families.is_empty() {
            "none".to_string()
        } else {
            self.adapter_families.join(",")
        };
        let adapter_formats = if self.adapter_formats.is_empty() {
            "none".to_string()
        } else {
            self.adapter_formats.join(",")
        };
        let validator_policy_refs = if self.validator_policy_refs.is_empty() {
            "none".to_string()
        } else {
            self.validator_policy_refs.join(",")
        };
        let memory_summary = self
            .available_memory_gb
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        let minimum_memory = self
            .minimum_memory_gb
            .map(|value| value.to_string())
            .unwrap_or_else(|| "n/a".to_string());
        let settlement_trigger = self
            .settlement_trigger
            .map(ProviderAdapterTrainingSettlementTrigger::label)
            .unwrap_or("none");
        format!(
            "backend=psionic_train execution=training family=adapter_training_contributor contributor_supported={} coordinator_match_supported={} authority_receipt_supported={} execution_backends={execution_backends} adapter_families={adapter_families} adapter_formats={adapter_formats} validator_policy_refs={validator_policy_refs} available_memory_gb={memory_summary} minimum_memory_gb={minimum_memory} settlement_trigger={settlement_trigger}",
            self.contributor_supported,
            self.coordinator_match_supported,
            self.authority_receipt_supported,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAdapterTrainingMatchRequest {
    pub training_run_id: String,
    pub adapter_family: String,
    pub adapter_format: String,
    pub validator_policy_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum_memory_gb: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_backend: Option<ProviderAdapterTrainingExecutionBackend>,
    pub settlement_trigger: ProviderAdapterTrainingSettlementTrigger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAdapterTrainingMatchReasonCode {
    ContributorUnavailable,
    CoordinatorMatchUnavailable,
    AuthorityReceiptsUnavailable,
    ExecutionBackendUnsupported,
    AdapterFamilyUnsupported,
    AdapterFormatUnsupported,
    ValidatorPolicyUnsupported,
    EnvironmentUnsupported,
    CheckpointFamilyUnsupported,
    MemoryInsufficient,
    SettlementTriggerUnsupported,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAdapterTrainingMatchVerdict {
    #[serde(default)]
    pub eligible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reason_codes: Vec<ProviderAdapterTrainingMatchReasonCode>,
}

pub fn match_adapter_training_contributor(
    availability: &ProviderAdapterTrainingContributorAvailability,
    request: &ProviderAdapterTrainingMatchRequest,
) -> ProviderAdapterTrainingMatchVerdict {
    let mut reason_codes = Vec::new();
    if !availability.contributor_supported {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::ContributorUnavailable);
    }
    if !availability.coordinator_match_supported {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::CoordinatorMatchUnavailable);
    }
    if !availability.authority_receipt_supported {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::AuthorityReceiptsUnavailable);
    }
    if request.execution_backend.is_some_and(|backend| {
        !availability
            .execution_backends
            .iter()
            .any(|value| *value == backend)
    }) {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::ExecutionBackendUnsupported);
    }
    if !availability
        .adapter_families
        .iter()
        .any(|value| value == &request.adapter_family)
    {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::AdapterFamilyUnsupported);
    }
    if !availability
        .adapter_formats
        .iter()
        .any(|value| value == &request.adapter_format)
    {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::AdapterFormatUnsupported);
    }
    if !availability
        .validator_policy_refs
        .iter()
        .any(|value| value == &request.validator_policy_ref)
    {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::ValidatorPolicyUnsupported);
    }
    if request
        .environment_ref
        .as_ref()
        .is_some_and(|environment_ref| {
            !availability.environment_refs.is_empty()
                && !availability
                    .environment_refs
                    .iter()
                    .any(|value| value == environment_ref)
        })
    {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::EnvironmentUnsupported);
    }
    if request
        .checkpoint_family
        .as_ref()
        .is_some_and(|checkpoint_family| {
            !availability.checkpoint_families.is_empty()
                && !availability
                    .checkpoint_families
                    .iter()
                    .any(|value| value == checkpoint_family)
        })
    {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::CheckpointFamilyUnsupported);
    }
    if request.minimum_memory_gb.is_some_and(|minimum_memory| {
        availability
            .available_memory_gb
            .is_some_and(|available_memory| available_memory < minimum_memory)
    }) {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::MemoryInsufficient);
    }
    if availability.settlement_trigger != Some(request.settlement_trigger) {
        reason_codes.push(ProviderAdapterTrainingMatchReasonCode::SettlementTriggerUnsupported);
    }
    ProviderAdapterTrainingMatchVerdict {
        eligible: reason_codes.is_empty(),
        reason_codes,
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAdapterTrainingSettlementHook {
    pub training_run_id: String,
    pub window_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contribution_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_outcome_id: Option<String>,
    pub trigger: ProviderAdapterTrainingSettlementTrigger,
    pub validator_policy_ref: String,
    pub window_summary_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validator_receipt_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion_receipt_digest: Option<String>,
}

pub fn settlement_hook_from_authority(
    window: &ComputeAdapterTrainingWindow,
    contribution: Option<&ComputeAdapterContributionOutcome>,
    trigger: ProviderAdapterTrainingSettlementTrigger,
) -> Option<ProviderAdapterTrainingSettlementHook> {
    match trigger {
        ProviderAdapterTrainingSettlementTrigger::AcceptedContribution => {
            let contribution = contribution?;
            if contribution.window_id != window.window_id
                || contribution.training_run_id != window.training_run_id
                || contribution.validator_disposition
                    != ComputeAdapterContributionDisposition::Accepted
                || contribution.aggregation_eligibility
                    != ComputeAdapterAggregationEligibility::Eligible
                || !contribution.accepted_for_aggregation
            {
                return None;
            }
            Some(ProviderAdapterTrainingSettlementHook {
                training_run_id: contribution.training_run_id.clone(),
                window_id: contribution.window_id.clone(),
                contribution_id: Some(contribution.contribution_id.clone()),
                accepted_outcome_id: window.accepted_outcome_id.clone(),
                trigger,
                validator_policy_ref: contribution.validator_policy_ref.clone(),
                window_summary_digest: window.window_summary_digest.clone(),
                validator_receipt_digest: Some(contribution.validator_receipt_digest.clone()),
                promotion_receipt_digest: contribution.promotion_receipt_digest.clone(),
            })
        }
        ProviderAdapterTrainingSettlementTrigger::AcceptedSealedWindow => {
            if window.accepted_outcome_id.is_none() {
                return None;
            }
            Some(ProviderAdapterTrainingSettlementHook {
                training_run_id: window.training_run_id.clone(),
                window_id: window.window_id.clone(),
                contribution_id: None,
                accepted_outcome_id: window.accepted_outcome_id.clone(),
                trigger,
                validator_policy_ref: window.validator_policy_ref.clone(),
                window_summary_digest: window.window_summary_digest.clone(),
                validator_receipt_digest: None,
                promotion_receipt_digest: None,
            })
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAvailability {
    #[serde(alias = "gpt_oss", alias = "ollama")]
    pub local_gemma: ProviderBackendHealth,
    #[serde(default, skip_serializing_if = "ProviderBackendHealth::is_inert")]
    pub apple_foundation_models: ProviderBackendHealth,
    #[serde(
        default,
        skip_serializing_if = "ProviderAppleAdapterHostingAvailability::is_inert"
    )]
    pub apple_adapter_hosting: ProviderAppleAdapterHostingAvailability,
    #[serde(
        default,
        skip_serializing_if = "ProviderAdapterTrainingContributorAvailability::is_inert"
    )]
    pub adapter_training_contributor: ProviderAdapterTrainingContributorAvailability,
    #[serde(
        default,
        skip_serializing_if = "ProviderPooledInferenceAvailability::is_inert"
    )]
    pub pooled_inference: ProviderPooledInferenceAvailability,
    pub sandbox: ProviderSandboxAvailability,
}

impl ProviderAvailability {
    pub fn active_inference_backend(&self) -> Option<ProviderBackendKind> {
        if self.apple_foundation_models.is_ready() {
            Some(ProviderBackendKind::AppleFoundationModels)
        } else if self.local_gemma.is_ready() {
            Some(ProviderBackendKind::GptOss)
        } else {
            None
        }
    }

    pub fn execution_backend_label(&self) -> &'static str {
        self.active_inference_backend()
            .map(ProviderBackendKind::label)
            .unwrap_or("no active inference backend")
    }

    pub fn product_visible(&self, product: ProviderComputeProduct) -> bool {
        match product {
            ProviderComputeProduct::GptOssInference
            | ProviderComputeProduct::AppleFoundationModelsInference => true,
            ProviderComputeProduct::PooledInferenceRemoteWholeRequest
            | ProviderComputeProduct::PooledInferenceReplicatedServing
            | ProviderComputeProduct::PooledInferenceDenseSplit
            | ProviderComputeProduct::PooledInferenceSparseExpert => {
                self.pooled_inference.has_authoritative_state()
            }
            ProviderComputeProduct::AppleFoundationModelsAdapterHosting => {
                self.apple_foundation_models.reachable
                    || self.apple_adapter_hosting.has_authoritative_state()
            }
            ProviderComputeProduct::AdapterTrainingContributor => {
                self.adapter_training_contributor.has_authoritative_state()
            }
            ProviderComputeProduct::GptOssEmbeddings => false,
            ProviderComputeProduct::SandboxContainerExec => self
                .sandbox
                .has_declared_execution_class(ProviderSandboxExecutionClass::ContainerExec),
            ProviderComputeProduct::SandboxPythonExec => self
                .sandbox
                .has_declared_execution_class(ProviderSandboxExecutionClass::PythonExec),
            ProviderComputeProduct::SandboxNodeExec => self
                .sandbox
                .has_declared_execution_class(ProviderSandboxExecutionClass::NodeExec),
            ProviderComputeProduct::SandboxPosixExec => self
                .sandbox
                .has_declared_execution_class(ProviderSandboxExecutionClass::PosixExec),
        }
    }

    pub fn product_backend_ready(&self, product: ProviderComputeProduct) -> bool {
        match product {
            ProviderComputeProduct::GptOssInference => self.local_gemma.is_ready(),
            ProviderComputeProduct::GptOssEmbeddings => false,
            ProviderComputeProduct::PooledInferenceRemoteWholeRequest => {
                self.pooled_inference.remote_whole_request_ready()
            }
            ProviderComputeProduct::PooledInferenceReplicatedServing => {
                self.pooled_inference.replicated_serving_ready()
            }
            ProviderComputeProduct::PooledInferenceDenseSplit => {
                self.pooled_inference.dense_split_ready()
            }
            ProviderComputeProduct::PooledInferenceSparseExpert => {
                self.pooled_inference.sparse_expert_ready()
            }
            ProviderComputeProduct::AppleFoundationModelsInference => {
                self.apple_foundation_models.is_ready()
            }
            ProviderComputeProduct::AppleFoundationModelsAdapterHosting => self
                .apple_adapter_hosting
                .product_backend_ready(&self.apple_foundation_models),
            ProviderComputeProduct::AdapterTrainingContributor => {
                self.adapter_training_contributor.product_backend_ready()
            }
            ProviderComputeProduct::SandboxContainerExec => self
                .sandbox
                .backend_ready_for_class(ProviderSandboxExecutionClass::ContainerExec),
            ProviderComputeProduct::SandboxPythonExec => self
                .sandbox
                .backend_ready_for_class(ProviderSandboxExecutionClass::PythonExec),
            ProviderComputeProduct::SandboxNodeExec => self
                .sandbox
                .backend_ready_for_class(ProviderSandboxExecutionClass::NodeExec),
            ProviderComputeProduct::SandboxPosixExec => self
                .sandbox
                .backend_ready_for_class(ProviderSandboxExecutionClass::PosixExec),
        }
    }

    pub fn capability_summary_for_product(&self, product: ProviderComputeProduct) -> String {
        match product {
            ProviderComputeProduct::SandboxContainerExec => self
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::ContainerExec)
                .unwrap_or_else(|| product.capability_summary_base().to_string()),
            ProviderComputeProduct::SandboxPythonExec => self
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::PythonExec)
                .unwrap_or_else(|| product.capability_summary_base().to_string()),
            ProviderComputeProduct::SandboxNodeExec => self
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::NodeExec)
                .unwrap_or_else(|| product.capability_summary_base().to_string()),
            ProviderComputeProduct::SandboxPosixExec => self
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::PosixExec)
                .unwrap_or_else(|| product.capability_summary_base().to_string()),
            ProviderComputeProduct::GptOssInference
            | ProviderComputeProduct::GptOssEmbeddings
            | ProviderComputeProduct::PooledInferenceRemoteWholeRequest
            | ProviderComputeProduct::PooledInferenceReplicatedServing
            | ProviderComputeProduct::PooledInferenceDenseSplit
            | ProviderComputeProduct::PooledInferenceSparseExpert
            | ProviderComputeProduct::AppleFoundationModelsInference
            | ProviderComputeProduct::AppleFoundationModelsAdapterHosting
            | ProviderComputeProduct::AdapterTrainingContributor => {
                product.capability_summary(self)
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderPooledInferenceTargetStatus {
    pub model: String,
    pub family: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supported_endpoints: Vec<String>,
    #[serde(default)]
    pub structured_outputs: bool,
    #[serde(default)]
    pub tool_calling: bool,
    #[serde(default)]
    pub response_state: bool,
    #[serde(default)]
    pub warm_replica_count: usize,
    #[serde(default)]
    pub local_warm_replica: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cluster_execution_modes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cluster_execution_topologies: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participating_workers: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderPooledInferenceAvailability {
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub management_base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topology_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default)]
    pub membership_state: String,
    #[serde(default)]
    pub member_count: usize,
    #[serde(default)]
    pub warm_replica_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_worker_id: Option<String>,
    #[serde(default)]
    pub local_serving_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served_mesh_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub served_mesh_posture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_posture: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targetable_models: Vec<ProviderPooledInferenceTargetStatus>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderProductMarketSemantics {
    pub contribution_class: String,
    pub market_receipt_class: String,
    pub earnings_trigger: String,
    pub revenue_posture: String,
    pub revenue_summary: String,
}

const POOLED_REMOTE_WHOLE_REQUEST_MODE: &str = "remote_whole_request";
const POOLED_REPLICATED_MODE: &str = "replicated";
const POOLED_DENSE_SPLIT_MODE: &str = "dense_split";
const POOLED_SPARSE_EXPERT_MODE: &str = "sparse_expert";

impl ProviderPooledInferenceTargetStatus {
    fn supports_execution_mode(&self, mode: &str) -> bool {
        self.cluster_execution_modes
            .iter()
            .any(|existing| existing == mode)
            || matches!(mode, POOLED_REPLICATED_MODE) && self.warm_replica_count > 1
    }
}

impl ProviderPooledInferenceAvailability {
    pub fn has_authoritative_state(&self) -> bool {
        let source_authoritative = !matches!(self.source.trim(), "" | "not_configured");
        let membership_authoritative = !matches!(self.membership_state.trim(), "" | "unconfigured");
        self.available
            || source_authoritative
            || self.management_base_url.is_some()
            || self.topology_digest.is_some()
            || self.default_model.is_some()
            || membership_authoritative
            || self.member_count > 0
            || self.warm_replica_count > 0
            || !self.targetable_models.is_empty()
    }

    pub fn is_inert(&self) -> bool {
        !self.has_authoritative_state()
    }

    pub fn remote_whole_request_ready(&self) -> bool {
        self.available
            && self.member_count > 0
            && !self.targetable_models.is_empty()
            && (self.targetable_models_support_mode(POOLED_REMOTE_WHOLE_REQUEST_MODE)
                || self.execution_mode.as_deref() == Some("proxy")
                || self.local_serving_state == "proxying")
    }

    pub fn replicated_serving_ready(&self) -> bool {
        self.available && self.targetable_models_support_mode(POOLED_REPLICATED_MODE)
    }

    pub fn dense_split_ready(&self) -> bool {
        self.available && self.targetable_models_support_mode(POOLED_DENSE_SPLIT_MODE)
    }

    pub fn sparse_expert_ready(&self) -> bool {
        self.available && self.targetable_models_support_mode(POOLED_SPARSE_EXPERT_MODE)
    }

    fn target_model_summary(&self) -> String {
        if self.targetable_models.is_empty() {
            return "none".to_string();
        }
        self.targetable_models
            .iter()
            .map(|target| {
                let execution_modes = if target.cluster_execution_modes.is_empty() {
                    "unspecified".to_string()
                } else {
                    target.cluster_execution_modes.join("+")
                };
                let execution_topologies = if target.cluster_execution_topologies.is_empty() {
                    "unspecified".to_string()
                } else {
                    target.cluster_execution_topologies.join("+")
                };
                let participating_workers = if target.participating_workers.is_empty() {
                    "unspecified".to_string()
                } else {
                    target.participating_workers.join("+")
                };
                format!(
                    "{}:{}:{}:{}:{}:{}",
                    target.model,
                    target.family,
                    execution_modes,
                    execution_topologies,
                    participating_workers,
                    target.warm_replica_count
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn targetable_models_support_mode(&self, mode: &str) -> bool {
        self.targetable_models
            .iter()
            .any(|target| target.supports_execution_mode(mode))
    }

    fn mode_model_summary(&self, mode: &str) -> String {
        let mut models = self
            .targetable_models
            .iter()
            .filter(|target| target.supports_execution_mode(mode))
            .map(|target| target.model.clone())
            .collect::<Vec<_>>();
        models.sort();
        models.dedup();
        if models.is_empty() {
            "none".to_string()
        } else {
            models.join(",")
        }
    }

    fn mode_topology_summary(&self, mode: &str) -> String {
        let mut topologies = self
            .targetable_models
            .iter()
            .filter(|target| target.supports_execution_mode(mode))
            .flat_map(|target| target.cluster_execution_topologies.iter().cloned())
            .collect::<Vec<_>>();
        topologies.sort();
        topologies.dedup();
        if topologies.is_empty() {
            "none".to_string()
        } else {
            topologies.join(",")
        }
    }

    fn mode_worker_summary(&self, mode: &str) -> String {
        let mut workers = self
            .targetable_models
            .iter()
            .filter(|target| target.supports_execution_mode(mode))
            .flat_map(|target| target.participating_workers.iter().cloned())
            .collect::<Vec<_>>();
        workers.sort();
        workers.dedup();
        if workers.is_empty() {
            "none".to_string()
        } else {
            workers.join(",")
        }
    }

    pub fn capability_summary(&self, mode_label: &str) -> String {
        let default_model = self.default_model.as_deref().unwrap_or("none");
        let topology_digest = self.topology_digest.as_deref().unwrap_or("none");
        let local_serving_state = if self.local_serving_state.trim().is_empty() {
            "unknown"
        } else {
            self.local_serving_state.as_str()
        };
        let execution_mode = self.execution_mode.as_deref().unwrap_or("unknown");
        let execution_engine = self.execution_engine.as_deref().unwrap_or("unknown");
        let fallback_posture = self.fallback_posture.as_deref().unwrap_or("none");
        let role = self.served_mesh_role.as_deref().unwrap_or("unknown");
        let posture = self.served_mesh_posture.as_deref().unwrap_or("unknown");
        format!(
            "backend=pooled_inference execution=clustered_inference family=inference mode={mode_label} source={} membership_state={} members={} warm_replicas={} default_model={default_model} topology_digest={topology_digest} local_state={local_serving_state} role={role} posture={posture} execution_mode={execution_mode} execution_engine={execution_engine} fallback_posture={fallback_posture} target_models={} mode_models={} mode_topologies={} mode_workers={}",
            self.source,
            self.membership_state,
            self.member_count,
            self.warm_replica_count,
            self.target_model_summary(),
            self.mode_model_summary(mode_label),
            self.mode_topology_summary(mode_label),
            self.mode_worker_summary(mode_label),
        )
    }

    fn market_semantics(&self, mode_label: &str) -> ProviderProductMarketSemantics {
        match mode_label {
            POOLED_REMOTE_WHOLE_REQUEST_MODE => {
                let revenue_posture = if self.remote_whole_request_ready() {
                    "serving_ready"
                } else {
                    "not_yet_earning"
                };
                let revenue_summary = if !self.has_authoritative_state() {
                    "No pooled revenue yet because mesh management is not configured."
                } else if self.targetable_models.is_empty() {
                    "No pooled revenue yet because the mesh has no targetable models."
                } else if self.member_count == 0 {
                    "No pooled revenue yet because the mesh has no joined members."
                } else if self.remote_whole_request_ready() {
                    "Earns when one machine serves the whole request on behalf of the pool and the clustered delivery proof is accepted."
                } else {
                    "No pooled revenue yet because remote whole-request serving is not ready."
                };
                ProviderProductMarketSemantics {
                    contribution_class: "remote_whole_request_serving".to_string(),
                    market_receipt_class: "clustered_delivery".to_string(),
                    earnings_trigger: "wallet_settled_accepted_clustered_delivery".to_string(),
                    revenue_posture: revenue_posture.to_string(),
                    revenue_summary: revenue_summary.to_string(),
                }
            }
            POOLED_REPLICATED_MODE => {
                let revenue_posture = if self.replicated_serving_ready() {
                    "standby_capacity_visible"
                } else {
                    "not_yet_earning"
                };
                let revenue_summary = if !self.has_authoritative_state() {
                    "No standby revenue yet because mesh management is not configured."
                } else if self
                    .targetable_models
                    .iter()
                    .all(|target| !target.supports_execution_mode(POOLED_REPLICATED_MODE))
                {
                    "No standby revenue yet because the pool has not warmed multiple replicas for any model."
                } else if self.replicated_serving_ready() {
                    "Warm replicas publish standby capacity, but revenue starts only when a reserve window sells or one replica is promoted into accepted delivery."
                } else {
                    "No standby revenue yet because the visible replica capacity is not ready to advertise."
                };
                ProviderProductMarketSemantics {
                    contribution_class: "replicated_standby_capacity".to_string(),
                    market_receipt_class: "clustered_standby_or_promoted_delivery".to_string(),
                    earnings_trigger: "accepted_reserved_window_or_promoted_delivery".to_string(),
                    revenue_posture: revenue_posture.to_string(),
                    revenue_summary: revenue_summary.to_string(),
                }
            }
            POOLED_DENSE_SPLIT_MODE => {
                let revenue_posture = if self.dense_split_ready() {
                    "serving_ready"
                } else {
                    "not_yet_earning"
                };
                let revenue_summary = if !self.has_authoritative_state() {
                    "No split-inference revenue yet because mesh management is not configured."
                } else if !self.targetable_models_support_mode(POOLED_DENSE_SPLIT_MODE) {
                    "No split-inference revenue yet because the pool has not admitted any split-across-machines model."
                } else if self.dense_split_ready() {
                    "Earns when one dense model request is actually split across multiple machines and the clustered delivery proof is accepted."
                } else {
                    "No split-inference revenue yet because split-across-machines serving is not ready."
                };
                ProviderProductMarketSemantics {
                    contribution_class: "dense_split_serving".to_string(),
                    market_receipt_class: "clustered_delivery".to_string(),
                    earnings_trigger: "wallet_settled_accepted_clustered_delivery".to_string(),
                    revenue_posture: revenue_posture.to_string(),
                    revenue_summary: revenue_summary.to_string(),
                }
            }
            POOLED_SPARSE_EXPERT_MODE => {
                let revenue_posture = if self.sparse_expert_ready() {
                    "serving_ready"
                } else {
                    "not_yet_earning"
                };
                let revenue_summary = if !self.has_authoritative_state() {
                    "No expert-sharded revenue yet because mesh management is not configured."
                } else if !self.targetable_models_support_mode(POOLED_SPARSE_EXPERT_MODE) {
                    "No expert-sharded revenue yet because the pool has not admitted any expert-sharded model."
                } else if self.sparse_expert_ready() {
                    "Earns when an expert-sharded request is executed across the pool and the clustered delivery proof is accepted."
                } else {
                    "No expert-sharded revenue yet because the admitted expert-sharded path is not ready to advertise."
                };
                ProviderProductMarketSemantics {
                    contribution_class: "sparse_expert_serving".to_string(),
                    market_receipt_class: "clustered_delivery".to_string(),
                    earnings_trigger: "wallet_settled_accepted_clustered_delivery".to_string(),
                    revenue_posture: revenue_posture.to_string(),
                    revenue_summary: revenue_summary.to_string(),
                }
            }
            _ => ProviderProductMarketSemantics {
                contribution_class: "clustered_inference".to_string(),
                market_receipt_class: "clustered_delivery".to_string(),
                earnings_trigger: "accepted_clustered_delivery".to_string(),
                revenue_posture: "unknown".to_string(),
                revenue_summary:
                    "Clustered inference revenue semantics are not specified for this topology."
                        .to_string(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderComputeProduct {
    GptOssInference,
    GptOssEmbeddings,
    PooledInferenceRemoteWholeRequest,
    PooledInferenceReplicatedServing,
    PooledInferenceDenseSplit,
    PooledInferenceSparseExpert,
    AppleFoundationModelsInference,
    AppleFoundationModelsAdapterHosting,
    AdapterTrainingContributor,
    SandboxContainerExec,
    SandboxPythonExec,
    SandboxNodeExec,
    SandboxPosixExec,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderProductDescriptor {
    pub product_id: String,
    pub compute_family: String,
    pub backend_family: String,
    pub sandbox_execution_class: Option<String>,
}

impl ProviderComputeProduct {
    pub const fn all() -> [Self; 12] {
        [
            Self::GptOssInference,
            Self::PooledInferenceRemoteWholeRequest,
            Self::PooledInferenceReplicatedServing,
            Self::PooledInferenceDenseSplit,
            Self::PooledInferenceSparseExpert,
            Self::AppleFoundationModelsInference,
            Self::AppleFoundationModelsAdapterHosting,
            Self::AdapterTrainingContributor,
            Self::SandboxContainerExec,
            Self::SandboxPythonExec,
            Self::SandboxNodeExec,
            Self::SandboxPosixExec,
        ]
    }

    pub const fn product_id(self) -> &'static str {
        match self {
            Self::GptOssInference => "psionic.local.inference.gemma.single_node",
            Self::GptOssEmbeddings => "psionic.local.embeddings.gemma.single_node",
            Self::PooledInferenceRemoteWholeRequest => {
                "psionic.cluster.inference.pooled.remote_whole_request"
            }
            Self::PooledInferenceReplicatedServing => "psionic.cluster.inference.pooled.replicated",
            Self::PooledInferenceDenseSplit => "psionic.cluster.inference.pooled.dense_split",
            Self::PooledInferenceSparseExpert => "psionic.cluster.inference.pooled.sparse_expert",
            Self::AppleFoundationModelsInference => {
                "psionic.local.inference.apple_foundation_models.single_node"
            }
            Self::AppleFoundationModelsAdapterHosting => {
                "psionic.local.adapter_hosting.apple_foundation_models.single_node"
            }
            Self::AdapterTrainingContributor => {
                "psionic.cluster.training.adapter_contributor.cluster_attached"
            }
            Self::SandboxContainerExec => {
                "psionic.remote_sandbox.sandbox_execution.container_exec.sandbox_isolated"
            }
            Self::SandboxPythonExec => {
                "psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated"
            }
            Self::SandboxNodeExec => {
                "psionic.remote_sandbox.sandbox_execution.node_exec.sandbox_isolated"
            }
            Self::SandboxPosixExec => {
                "psionic.remote_sandbox.sandbox_execution.posix_exec.sandbox_isolated"
            }
        }
    }

    pub const fn display_label(self) -> &'static str {
        match self {
            Self::GptOssInference => "Gemma inference",
            Self::GptOssEmbeddings => "Gemma embeddings",
            Self::PooledInferenceRemoteWholeRequest => "Pooled remote serving",
            Self::PooledInferenceReplicatedServing => "Pooled standby replicas",
            Self::PooledInferenceDenseSplit => "Pooled split inference",
            Self::PooledInferenceSparseExpert => "Pooled expert-sharded inference",
            Self::AppleFoundationModelsInference => "Apple FM inference",
            Self::AppleFoundationModelsAdapterHosting => "Apple FM adapter hosting",
            Self::AdapterTrainingContributor => "Adapter training contributor",
            Self::SandboxContainerExec => "Sandbox container exec",
            Self::SandboxPythonExec => "Sandbox python exec",
            Self::SandboxNodeExec => "Sandbox node exec",
            Self::SandboxPosixExec => "Sandbox posix exec",
        }
    }

    pub const fn backend_kind(self) -> Option<ProviderBackendKind> {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => Some(ProviderBackendKind::GptOss),
            Self::AppleFoundationModelsInference | Self::AppleFoundationModelsAdapterHosting => {
                Some(ProviderBackendKind::AppleFoundationModels)
            }
            Self::AdapterTrainingContributor => Some(ProviderBackendKind::PsionicTrain),
            Self::PooledInferenceRemoteWholeRequest
            | Self::PooledInferenceReplicatedServing
            | Self::PooledInferenceDenseSplit
            | Self::PooledInferenceSparseExpert => None,
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => None,
        }
    }

    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => "local_gemma",
            Self::AppleFoundationModelsInference | Self::AppleFoundationModelsAdapterHosting => {
                "apple_foundation_models"
            }
            Self::AdapterTrainingContributor => "psionic_train",
            Self::PooledInferenceRemoteWholeRequest
            | Self::PooledInferenceReplicatedServing
            | Self::PooledInferenceDenseSplit
            | Self::PooledInferenceSparseExpert => "pooled_inference",
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "sandbox",
        }
    }

    pub const fn compute_family_label(self) -> &'static str {
        match self {
            Self::GptOssInference
            | Self::PooledInferenceRemoteWholeRequest
            | Self::PooledInferenceReplicatedServing
            | Self::PooledInferenceDenseSplit
            | Self::PooledInferenceSparseExpert
            | Self::AppleFoundationModelsInference => "inference",
            Self::AppleFoundationModelsAdapterHosting => "adapter_hosting",
            Self::AdapterTrainingContributor => "training",
            Self::GptOssEmbeddings => "embeddings",
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "sandbox_execution",
        }
    }

    pub const fn sandbox_execution_class(self) -> Option<ProviderSandboxExecutionClass> {
        match self {
            Self::SandboxContainerExec => Some(ProviderSandboxExecutionClass::ContainerExec),
            Self::SandboxPythonExec => Some(ProviderSandboxExecutionClass::PythonExec),
            Self::SandboxNodeExec => Some(ProviderSandboxExecutionClass::NodeExec),
            Self::SandboxPosixExec => Some(ProviderSandboxExecutionClass::PosixExec),
            Self::GptOssInference
            | Self::GptOssEmbeddings
            | Self::PooledInferenceRemoteWholeRequest
            | Self::PooledInferenceReplicatedServing
            | Self::PooledInferenceDenseSplit
            | Self::PooledInferenceSparseExpert
            | Self::AppleFoundationModelsInference
            | Self::AppleFoundationModelsAdapterHosting
            | Self::AdapterTrainingContributor => None,
        }
    }

    pub const fn capability_summary_base(self) -> &'static str {
        match self {
            Self::GptOssInference => {
                "backend=local_gemma execution=local_inference family=inference"
            }
            Self::GptOssEmbeddings => {
                "backend=local_gemma execution=local_inference family=embeddings status=unsupported"
            }
            Self::PooledInferenceRemoteWholeRequest => {
                "backend=pooled_inference execution=clustered_inference family=inference mode=remote_whole_request"
            }
            Self::PooledInferenceReplicatedServing => {
                "backend=pooled_inference execution=clustered_inference family=inference mode=replicated"
            }
            Self::PooledInferenceDenseSplit => {
                "backend=pooled_inference execution=clustered_inference family=inference mode=dense_split"
            }
            Self::PooledInferenceSparseExpert => {
                "backend=pooled_inference execution=clustered_inference family=inference mode=sparse_expert"
            }
            Self::AppleFoundationModelsInference => {
                "backend=apple_foundation_models execution=local_inference family=inference apple_silicon=true apple_intelligence=true"
            }
            Self::AppleFoundationModelsAdapterHosting => {
                "backend=apple_foundation_models execution=local_inference family=adapter_hosting apple_silicon=true apple_intelligence=true"
            }
            Self::AdapterTrainingContributor => {
                "backend=psionic_train execution=training family=adapter_training_contributor"
            }
            Self::SandboxContainerExec => {
                "backend=sandbox execution=sandbox.container.exec family=sandbox_execution"
            }
            Self::SandboxPythonExec => {
                "backend=sandbox execution=sandbox.python.exec family=sandbox_execution"
            }
            Self::SandboxNodeExec => {
                "backend=sandbox execution=sandbox.node.exec family=sandbox_execution"
            }
            Self::SandboxPosixExec => {
                "backend=sandbox execution=sandbox.posix.exec family=sandbox_execution"
            }
        }
    }

    pub fn capability_summary(self, availability: &ProviderAvailability) -> String {
        let base_summary = self.capability_summary_base();
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => {
                let health = &availability.local_gemma;
                let ready_model = health.ready_model.as_deref().unwrap_or("none");
                let configured_model = health.configured_model.as_deref().unwrap_or("none");
                let latency_ms = health
                    .latency_ms_p50
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "n/a".to_string());
                format!(
                    "{base_summary} model={ready_model} configured_model={configured_model} latency_ms_p50={latency_ms}"
                )
            }
            Self::PooledInferenceRemoteWholeRequest => {
                let capability = availability
                    .pooled_inference
                    .capability_summary(POOLED_REMOTE_WHOLE_REQUEST_MODE);
                let market = availability
                    .pooled_inference
                    .market_semantics(POOLED_REMOTE_WHOLE_REQUEST_MODE);
                format!(
                    "{capability} contribution_class={} market_receipt_class={} earnings_trigger={} revenue_posture={} revenue_summary={}",
                    market.contribution_class,
                    market.market_receipt_class,
                    market.earnings_trigger,
                    market.revenue_posture,
                    canonical_market_summary_token(market.revenue_summary.as_str()),
                )
            }
            Self::PooledInferenceReplicatedServing => {
                let capability = availability
                    .pooled_inference
                    .capability_summary(POOLED_REPLICATED_MODE);
                let market = availability
                    .pooled_inference
                    .market_semantics(POOLED_REPLICATED_MODE);
                format!(
                    "{capability} contribution_class={} market_receipt_class={} earnings_trigger={} revenue_posture={} revenue_summary={}",
                    market.contribution_class,
                    market.market_receipt_class,
                    market.earnings_trigger,
                    market.revenue_posture,
                    canonical_market_summary_token(market.revenue_summary.as_str()),
                )
            }
            Self::PooledInferenceDenseSplit => {
                let capability = availability
                    .pooled_inference
                    .capability_summary(POOLED_DENSE_SPLIT_MODE);
                let market = availability
                    .pooled_inference
                    .market_semantics(POOLED_DENSE_SPLIT_MODE);
                format!(
                    "{capability} contribution_class={} market_receipt_class={} earnings_trigger={} revenue_posture={} revenue_summary={}",
                    market.contribution_class,
                    market.market_receipt_class,
                    market.earnings_trigger,
                    market.revenue_posture,
                    canonical_market_summary_token(market.revenue_summary.as_str()),
                )
            }
            Self::PooledInferenceSparseExpert => {
                let capability = availability
                    .pooled_inference
                    .capability_summary(POOLED_SPARSE_EXPERT_MODE);
                let market = availability
                    .pooled_inference
                    .market_semantics(POOLED_SPARSE_EXPERT_MODE);
                format!(
                    "{capability} contribution_class={} market_receipt_class={} earnings_trigger={} revenue_posture={} revenue_summary={}",
                    market.contribution_class,
                    market.market_receipt_class,
                    market.earnings_trigger,
                    market.revenue_posture,
                    canonical_market_summary_token(market.revenue_summary.as_str()),
                )
            }
            Self::AppleFoundationModelsInference => {
                let health = &availability.apple_foundation_models;
                let ready_model = health.ready_model.as_deref().unwrap_or("none");
                let availability_message =
                    health.availability_message.as_deref().unwrap_or("ready");
                format!("{base_summary} model={ready_model} platform_gate={availability_message}")
            }
            Self::AppleFoundationModelsAdapterHosting => {
                let health = &availability.apple_foundation_models;
                let ready_model = health.ready_model.as_deref().unwrap_or("none");
                let availability_message =
                    health.availability_message.as_deref().unwrap_or("ready");
                let adapter_state = &availability.apple_adapter_hosting;
                let compatible_digests = adapter_state.compatible_package_digests();
                let compatible_digest_summary = if compatible_digests.is_empty() {
                    "none".to_string()
                } else {
                    compatible_digests.join(",")
                };
                format!(
                    "{base_summary} model={ready_model} platform_gate={availability_message} inventory_supported={} attach_supported={} loaded_adapters={} compatible_adapters={} compatible_adapter_digests={compatible_digest_summary}",
                    adapter_state.inventory_supported,
                    adapter_state.attach_supported,
                    adapter_state.loaded_adapter_count(),
                    adapter_state.compatible_adapter_count(),
                )
            }
            Self::AdapterTrainingContributor => availability
                .adapter_training_contributor
                .capability_summary(),
            Self::SandboxContainerExec => availability
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::ContainerExec)
                .unwrap_or_else(|| base_summary.to_string()),
            Self::SandboxPythonExec => availability
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::PythonExec)
                .unwrap_or_else(|| base_summary.to_string()),
            Self::SandboxNodeExec => availability
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::NodeExec)
                .unwrap_or_else(|| base_summary.to_string()),
            Self::SandboxPosixExec => availability
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::PosixExec)
                .unwrap_or_else(|| base_summary.to_string()),
        }
    }

    pub const fn terms_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => "spot session / local best effort",
            Self::PooledInferenceRemoteWholeRequest => "spot session / pooled remote serving",
            Self::PooledInferenceReplicatedServing => "spot session / pooled standby replicas",
            Self::PooledInferenceDenseSplit => {
                "spot session / pooled split-across-machines inference"
            }
            Self::PooledInferenceSparseExpert => "spot session / pooled expert-sharded inference",
            Self::AppleFoundationModelsInference => "spot session / Apple gated best effort",
            Self::AppleFoundationModelsAdapterHosting => {
                "spot session / Apple gated adapter best effort"
            }
            Self::AdapterTrainingContributor => {
                "window assignment / accepted contribution or accepted sealed window"
            }
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "spot session / declared sandbox profile",
        }
    }

    pub const fn forward_terms_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => {
                "forward physical / committed local window"
            }
            Self::PooledInferenceRemoteWholeRequest => {
                "forward physical / pooled remote serving window"
            }
            Self::PooledInferenceReplicatedServing => {
                "forward physical / pooled standby replica window"
            }
            Self::PooledInferenceDenseSplit => "forward physical / pooled split inference window",
            Self::PooledInferenceSparseExpert => "forward physical / pooled expert-sharded window",
            Self::AppleFoundationModelsInference => {
                "forward physical / Apple gated committed window"
            }
            Self::AppleFoundationModelsAdapterHosting => {
                "forward physical / Apple gated adapter window"
            }
            Self::AdapterTrainingContributor => {
                "forward physical / contributor window with authority receipts"
            }
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "forward physical / declared sandbox profile window",
        }
    }

    pub const fn default_price_floor_sats(self) -> u64 {
        match self {
            Self::GptOssInference => 21,
            Self::GptOssEmbeddings => 8,
            Self::PooledInferenceRemoteWholeRequest => 55,
            Self::PooledInferenceReplicatedServing => 89,
            Self::PooledInferenceDenseSplit => 144,
            Self::PooledInferenceSparseExpert => 233,
            Self::AppleFoundationModelsInference => 34,
            Self::AppleFoundationModelsAdapterHosting => 55,
            Self::AdapterTrainingContributor => 89,
            Self::SandboxContainerExec => 55,
            Self::SandboxPythonExec => 34,
            Self::SandboxNodeExec => 34,
            Self::SandboxPosixExec => 21,
        }
    }

    pub fn for_product_id(product_id: &str) -> Option<Self> {
        match product_id.trim() {
            "psionic.local.inference.gemma.single_node"
            | "psionic.local.inference.gpt_oss.single_node"
            | "local_gemma.text_generation"
            | "ollama.text_generation"
            | "gpt_oss.text_generation" => Some(Self::GptOssInference),
            "psionic.local.embeddings.gemma.single_node"
            | "psionic.local.embeddings.gpt_oss.single_node"
            | "local_gemma.embeddings"
            | "ollama.embeddings"
            | "gpt_oss.embeddings" => Some(Self::GptOssEmbeddings),
            "psionic.cluster.inference.pooled.remote_whole_request"
            | "psionic.cluster.inference.gpt_oss.remote_whole_request"
            | "gpt_oss.clustered_inference.remote_whole_request" => {
                Some(Self::PooledInferenceRemoteWholeRequest)
            }
            "psionic.cluster.inference.pooled.replicated"
            | "psionic.cluster.inference.gpt_oss.replicated"
            | "gpt_oss.clustered_inference.replicated" => {
                Some(Self::PooledInferenceReplicatedServing)
            }
            "psionic.cluster.inference.pooled.dense_split" => Some(Self::PooledInferenceDenseSplit),
            "psionic.cluster.inference.pooled.sparse_expert" => {
                Some(Self::PooledInferenceSparseExpert)
            }
            "psionic.local.inference.apple_foundation_models.single_node"
            | "apple_foundation_models.text_generation" => {
                Some(Self::AppleFoundationModelsInference)
            }
            "psionic.local.adapter_hosting.apple_foundation_models.single_node"
            | "apple_foundation_models.adapter_hosting" => {
                Some(Self::AppleFoundationModelsAdapterHosting)
            }
            "psionic.cluster.training.adapter_contributor.cluster_attached"
            | "adapter_training.contributor" => Some(Self::AdapterTrainingContributor),
            "psionic.remote_sandbox.sandbox_execution.container_exec.sandbox_isolated"
            | "sandbox.container.exec" => Some(Self::SandboxContainerExec),
            "psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated"
            | "sandbox.python.exec" => Some(Self::SandboxPythonExec),
            "psionic.remote_sandbox.sandbox_execution.node_exec.sandbox_isolated"
            | "sandbox.node.exec" => Some(Self::SandboxNodeExec),
            "psionic.remote_sandbox.sandbox_execution.posix_exec.sandbox_isolated"
            | "sandbox.posix.exec" => Some(Self::SandboxPosixExec),
            _ => None,
        }
    }

    pub fn descriptor(self) -> ProviderProductDescriptor {
        ProviderProductDescriptor {
            product_id: self.product_id().to_string(),
            compute_family: self.compute_family_label().to_string(),
            backend_family: self.backend_label().to_string(),
            sandbox_execution_class: self
                .sandbox_execution_class()
                .map(|execution_class| execution_class.product_id().to_string()),
        }
    }

    pub fn market_semantics(
        self,
        availability: &ProviderAvailability,
    ) -> ProviderProductMarketSemantics {
        match self {
            Self::GptOssInference => ProviderProductMarketSemantics {
                contribution_class: "single_node_serving".to_string(),
                market_receipt_class: "accepted_delivery".to_string(),
                earnings_trigger: "wallet_settled_accepted_delivery".to_string(),
                revenue_posture: if availability.local_gemma.is_ready() {
                    "serving_ready"
                } else {
                    "not_yet_earning"
                }
                .to_string(),
                revenue_summary: if availability.local_gemma.is_ready() {
                    "Earns when local delivery is accepted and wallet settlement is confirmed."
                } else {
                    "No single-node revenue yet because the local Gemma runtime is not ready."
                }
                .to_string(),
            },
            Self::AppleFoundationModelsInference => ProviderProductMarketSemantics {
                contribution_class: "single_node_serving".to_string(),
                market_receipt_class: "accepted_delivery".to_string(),
                earnings_trigger: "wallet_settled_accepted_delivery".to_string(),
                revenue_posture: if availability.apple_foundation_models.is_ready() {
                    "serving_ready"
                } else {
                    "not_yet_earning"
                }
                .to_string(),
                revenue_summary: if availability.apple_foundation_models.is_ready() {
                    "Earns when Apple FM delivery is accepted and wallet settlement is confirmed."
                } else {
                    "No Apple FM revenue yet because the local inference bridge is not ready."
                }
                .to_string(),
            },
            Self::PooledInferenceRemoteWholeRequest => availability
                .pooled_inference
                .market_semantics(POOLED_REMOTE_WHOLE_REQUEST_MODE),
            Self::PooledInferenceReplicatedServing => {
                availability
                    .pooled_inference
                    .market_semantics(POOLED_REPLICATED_MODE)
            }
            Self::PooledInferenceDenseSplit => availability
                .pooled_inference
                .market_semantics(POOLED_DENSE_SPLIT_MODE),
            Self::PooledInferenceSparseExpert => availability
                .pooled_inference
                .market_semantics(POOLED_SPARSE_EXPERT_MODE),
            Self::GptOssEmbeddings => ProviderProductMarketSemantics {
                contribution_class: "single_node_embedding".to_string(),
                market_receipt_class: "unsupported".to_string(),
                earnings_trigger: "unsupported".to_string(),
                revenue_posture: "unsupported".to_string(),
                revenue_summary: "Gemma embeddings are not a supported market lane.".to_string(),
            },
            Self::AppleFoundationModelsAdapterHosting => ProviderProductMarketSemantics {
                contribution_class: "single_node_adapter_hosting".to_string(),
                market_receipt_class: "best_effort_adapter_hosting".to_string(),
                earnings_trigger: "future_market_lane".to_string(),
                revenue_posture: "future_scope".to_string(),
                revenue_summary: "Adapter hosting is visible in the product but not a wallet-settled market lane yet.".to_string(),
            },
            Self::AdapterTrainingContributor => ProviderProductMarketSemantics {
                contribution_class: "training_contributor".to_string(),
                market_receipt_class: "accepted_window_or_sealed_contribution".to_string(),
                earnings_trigger: "accepted_training_window".to_string(),
                revenue_posture: "future_scope".to_string(),
                revenue_summary: "Training contributions settle on accepted windows, not immediate inference receipts.".to_string(),
            },
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => ProviderProductMarketSemantics {
                contribution_class: "sandbox_execution".to_string(),
                market_receipt_class: "accepted_delivery".to_string(),
                earnings_trigger: "wallet_settled_accepted_delivery".to_string(),
                revenue_posture: "conditional".to_string(),
                revenue_summary: "Sandbox execution earns only when a declared sandbox delivery is accepted and settled.".to_string(),
            },
        }
    }
}

fn canonical_market_summary_token(summary: &str) -> String {
    summary
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_")
        .to_ascii_lowercase()
}

pub fn describe_provider_product_id(product_id: &str) -> Option<ProviderProductDescriptor> {
    ProviderComputeProduct::for_product_id(product_id).map(ProviderComputeProduct::descriptor)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderInventoryRow {
    #[serde(
        serialize_with = "serialize_inventory_row_target",
        deserialize_with = "deserialize_inventory_row_target"
    )]
    pub target: ProviderComputeProduct,
    pub enabled: bool,
    pub backend_ready: bool,
    pub eligible: bool,
    pub capability_summary: String,
    pub market_receipt_class: String,
    pub earnings_summary: String,
    pub source_badge: String,
    pub capacity_lot_id: Option<String>,
    pub total_quantity: u64,
    pub reserved_quantity: u64,
    pub available_quantity: u64,
    pub delivery_state: String,
    pub price_floor_sats: u64,
    pub terms_label: String,
    pub forward_capacity_lot_id: Option<String>,
    pub forward_delivery_window_label: Option<String>,
    pub forward_total_quantity: u64,
    pub forward_reserved_quantity: u64,
    pub forward_available_quantity: u64,
    pub forward_terms_label: Option<String>,
}

fn serialize_inventory_row_target<S>(
    target: &ProviderComputeProduct,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(target.product_id())
}

fn deserialize_inventory_row_target<'de, D>(
    deserializer: D,
) -> Result<ProviderComputeProduct, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = String::deserialize(deserializer)?;
    if let Some(product) = ProviderComputeProduct::for_product_id(raw.as_str()) {
        return Ok(product);
    }
    match raw.as_str() {
        "gpt_oss_inference" => Ok(ProviderComputeProduct::GptOssInference),
        "gpt_oss_embeddings" => Ok(ProviderComputeProduct::GptOssEmbeddings),
        "pooled_inference_remote_whole_request" => {
            Ok(ProviderComputeProduct::PooledInferenceRemoteWholeRequest)
        }
        "pooled_inference_replicated_serving" => {
            Ok(ProviderComputeProduct::PooledInferenceReplicatedServing)
        }
        "pooled_inference_dense_split" => Ok(ProviderComputeProduct::PooledInferenceDenseSplit),
        "pooled_inference_sparse_expert" => Ok(ProviderComputeProduct::PooledInferenceSparseExpert),
        "apple_foundation_models_inference" => {
            Ok(ProviderComputeProduct::AppleFoundationModelsInference)
        }
        "apple_foundation_models_adapter_hosting" => {
            Ok(ProviderComputeProduct::AppleFoundationModelsAdapterHosting)
        }
        "adapter_training_contributor" => Ok(ProviderComputeProduct::AdapterTrainingContributor),
        "sandbox_container_exec" => Ok(ProviderComputeProduct::SandboxContainerExec),
        "sandbox_python_exec" => Ok(ProviderComputeProduct::SandboxPythonExec),
        "sandbox_node_exec" => Ok(ProviderComputeProduct::SandboxNodeExec),
        "sandbox_posix_exec" => Ok(ProviderComputeProduct::SandboxPosixExec),
        _ => Err(D::Error::custom(format!(
            "unknown provider inventory target '{raw}'"
        ))),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderInventoryControls {
    #[serde(
        alias = "gpt_oss_inference_enabled",
        alias = "ollama_inference_enabled"
    )]
    pub local_gemma_inference_enabled: bool,
    #[serde(
        alias = "gpt_oss_embeddings_enabled",
        alias = "ollama_embeddings_enabled"
    )]
    pub local_gemma_embeddings_enabled: bool,
    #[serde(default)]
    pub pooled_inference_remote_whole_request_enabled: bool,
    #[serde(default)]
    pub pooled_inference_replicated_serving_enabled: bool,
    #[serde(default)]
    pub pooled_inference_dense_split_enabled: bool,
    #[serde(default)]
    pub pooled_inference_sparse_expert_enabled: bool,
    pub apple_fm_inference_enabled: bool,
    #[serde(default)]
    pub apple_fm_adapter_hosting_enabled: bool,
    #[serde(default)]
    pub adapter_training_contributor_enabled: bool,
    pub sandbox_container_exec_enabled: bool,
    pub sandbox_python_exec_enabled: bool,
    pub sandbox_node_exec_enabled: bool,
    pub sandbox_posix_exec_enabled: bool,
}

impl Default for ProviderInventoryControls {
    fn default() -> Self {
        Self {
            local_gemma_inference_enabled: true,
            local_gemma_embeddings_enabled: false,
            pooled_inference_remote_whole_request_enabled: true,
            pooled_inference_replicated_serving_enabled: true,
            pooled_inference_dense_split_enabled: true,
            pooled_inference_sparse_expert_enabled: true,
            apple_fm_inference_enabled: true,
            apple_fm_adapter_hosting_enabled: true,
            adapter_training_contributor_enabled: false,
            sandbox_container_exec_enabled: false,
            sandbox_python_exec_enabled: false,
            sandbox_node_exec_enabled: false,
            sandbox_posix_exec_enabled: false,
        }
    }
}

impl ProviderInventoryControls {
    pub const fn is_advertised(&self, target: ProviderComputeProduct) -> bool {
        match target {
            ProviderComputeProduct::GptOssInference => self.local_gemma_inference_enabled,
            ProviderComputeProduct::GptOssEmbeddings => false,
            ProviderComputeProduct::PooledInferenceRemoteWholeRequest => {
                self.pooled_inference_remote_whole_request_enabled
            }
            ProviderComputeProduct::PooledInferenceReplicatedServing => {
                self.pooled_inference_replicated_serving_enabled
            }
            ProviderComputeProduct::PooledInferenceDenseSplit => {
                self.pooled_inference_dense_split_enabled
            }
            ProviderComputeProduct::PooledInferenceSparseExpert => {
                self.pooled_inference_sparse_expert_enabled
            }
            ProviderComputeProduct::AppleFoundationModelsInference => {
                self.apple_fm_inference_enabled
            }
            ProviderComputeProduct::AppleFoundationModelsAdapterHosting => {
                self.apple_fm_adapter_hosting_enabled
            }
            ProviderComputeProduct::AdapterTrainingContributor => {
                self.adapter_training_contributor_enabled
            }
            ProviderComputeProduct::SandboxContainerExec => self.sandbox_container_exec_enabled,
            ProviderComputeProduct::SandboxPythonExec => self.sandbox_python_exec_enabled,
            ProviderComputeProduct::SandboxNodeExec => self.sandbox_node_exec_enabled,
            ProviderComputeProduct::SandboxPosixExec => self.sandbox_posix_exec_enabled,
        }
    }

    pub fn is_product_advertised(&self, product_id: &str) -> bool {
        ProviderComputeProduct::for_product_id(product_id)
            .is_some_and(|target| self.is_advertised(target))
    }

    pub fn toggle(&mut self, target: ProviderComputeProduct) -> bool {
        let enabled = match target {
            ProviderComputeProduct::GptOssInference => &mut self.local_gemma_inference_enabled,
            ProviderComputeProduct::PooledInferenceRemoteWholeRequest => {
                &mut self.pooled_inference_remote_whole_request_enabled
            }
            ProviderComputeProduct::PooledInferenceReplicatedServing => {
                &mut self.pooled_inference_replicated_serving_enabled
            }
            ProviderComputeProduct::PooledInferenceDenseSplit => {
                &mut self.pooled_inference_dense_split_enabled
            }
            ProviderComputeProduct::PooledInferenceSparseExpert => {
                &mut self.pooled_inference_sparse_expert_enabled
            }
            ProviderComputeProduct::AppleFoundationModelsInference => {
                &mut self.apple_fm_inference_enabled
            }
            ProviderComputeProduct::AppleFoundationModelsAdapterHosting => {
                &mut self.apple_fm_adapter_hosting_enabled
            }
            ProviderComputeProduct::AdapterTrainingContributor => {
                &mut self.adapter_training_contributor_enabled
            }
            ProviderComputeProduct::SandboxContainerExec => {
                &mut self.sandbox_container_exec_enabled
            }
            ProviderComputeProduct::SandboxPythonExec => &mut self.sandbox_python_exec_enabled,
            ProviderComputeProduct::SandboxNodeExec => &mut self.sandbox_node_exec_enabled,
            ProviderComputeProduct::SandboxPosixExec => &mut self.sandbox_posix_exec_enabled,
            ProviderComputeProduct::GptOssEmbeddings => {
                self.local_gemma_embeddings_enabled = false;
                return false;
            }
        };
        *enabled = !*enabled;
        *enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAdvertisedProduct {
    pub product: ProviderComputeProduct,
    pub enabled: bool,
    pub backend_ready: bool,
    pub eligible: bool,
    pub capability_summary: String,
    pub market_receipt_class: String,
    pub earnings_summary: String,
    pub price_floor_sats: u64,
    pub terms_label: String,
    pub forward_terms_label: String,
}

pub fn derive_provider_products(
    availability: &ProviderAvailability,
    controls: &ProviderInventoryControls,
) -> Vec<ProviderAdvertisedProduct> {
    ProviderComputeProduct::all()
        .into_iter()
        .filter(|product| availability.product_visible(*product))
        .map(|product| {
            let enabled = controls.is_advertised(product);
            let backend_ready = availability.product_backend_ready(product);
            let market_semantics = product.market_semantics(availability);
            ProviderAdvertisedProduct {
                product,
                enabled,
                backend_ready,
                eligible: enabled && backend_ready,
                capability_summary: availability.capability_summary_for_product(product),
                market_receipt_class: market_semantics.market_receipt_class,
                earnings_summary: market_semantics.revenue_summary,
                price_floor_sats: product.default_price_floor_sats(),
                terms_label: product.terms_label().to_string(),
                forward_terms_label: product.forward_terms_label().to_string(),
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ProviderLifecycleInput<'a> {
    pub current_mode: ProviderMode,
    pub ingress_mode: ProviderIngressMode,
    pub relay_error: Option<&'a str>,
    pub availability: &'a ProviderAvailability,
    pub backend_unavailable_detail: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderLifecycleTransition {
    StayOffline,
    HoldCurrent,
    Degraded {
        reason_code: &'static str,
        error_detail: String,
        failure_class: ProviderFailureClass,
    },
    Online {
        active_backend: ProviderBackendKind,
    },
}

pub fn derive_provider_lifecycle(
    input: &ProviderLifecycleInput<'_>,
) -> ProviderLifecycleTransition {
    if input.current_mode == ProviderMode::Offline
        && input.ingress_mode != ProviderIngressMode::Online
    {
        return ProviderLifecycleTransition::StayOffline;
    }

    if let Some(relay_error) = input.relay_error {
        return ProviderLifecycleTransition::Degraded {
            reason_code: "NIP90_RELAY_INGRESS_ERROR",
            error_detail: relay_error.to_string(),
            failure_class: ProviderFailureClass::Relay,
        };
    }

    if input.ingress_mode != ProviderIngressMode::Online {
        return ProviderLifecycleTransition::HoldCurrent;
    }

    let Some(active_backend) = input.availability.active_inference_backend() else {
        return ProviderLifecycleTransition::Degraded {
            reason_code: "INFERENCE_BACKEND_UNAVAILABLE",
            error_detail: input
                .backend_unavailable_detail
                .unwrap_or("No local inference backend is ready")
                .to_string(),
            failure_class: ProviderFailureClass::Execution,
        };
    };

    if input.current_mode != ProviderMode::Offline {
        ProviderLifecycleTransition::Online { active_backend }
    } else {
        ProviderLifecycleTransition::HoldCurrent
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::{
        ProviderAdapterTrainingContributorAvailability, ProviderAdapterTrainingExecutionBackend,
        ProviderAdapterTrainingMatchReasonCode, ProviderAdapterTrainingMatchRequest,
        ProviderAdapterTrainingSettlementTrigger, ProviderAppleAdapterHostingAvailability,
        ProviderAppleAdapterHostingEntry, ProviderAvailability, ProviderBackendHealth,
        ProviderBackendKind, ProviderComputeProduct, ProviderFailureClass, ProviderIngressMode,
        ProviderInventoryControls, ProviderInventoryRow, ProviderLifecycleInput,
        ProviderLifecycleTransition, ProviderMode, ProviderPooledInferenceAvailability,
        ProviderPooledInferenceTargetStatus, ProviderSandboxAvailability,
        ProviderSandboxDetectionConfig, ProviderSandboxExecutionClass, ProviderSandboxProfileSpec,
        derive_provider_lifecycle, derive_provider_products, describe_provider_product_id,
        detect_sandbox_supply, match_adapter_training_contributor, settlement_hook_from_authority,
    };
    use openagents_kernel_core::compute::{
        ComputeAdapterAggregationEligibility, ComputeAdapterContributionDisposition,
        ComputeAdapterContributionOutcome, ComputeAdapterDatasetSlice,
        ComputeAdapterPolicyRevision, ComputeAdapterPromotionDisposition,
        ComputeAdapterTrainingWindow, ComputeAdapterWindowStatus,
    };

    fn ensure(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if condition {
            Ok(())
        } else {
            Err(std::io::Error::other(message.to_string()).into())
        }
    }

    fn ready_health(
        configured_model: Option<&str>,
        ready_model: &str,
        latency_ms_p50: Option<u64>,
    ) -> ProviderBackendHealth {
        ProviderBackendHealth {
            reachable: true,
            ready: true,
            configured_model: configured_model.map(str::to_string),
            ready_model: Some(ready_model.to_string()),
            available_models: vec![ready_model.to_string()],
            last_error: None,
            last_action: Some("ready".to_string()),
            availability_message: None,
            latency_ms_p50,
        }
    }

    fn ready_adapter_training_contributor() -> ProviderAdapterTrainingContributorAvailability {
        ProviderAdapterTrainingContributorAvailability {
            contributor_supported: true,
            coordinator_match_supported: true,
            authority_receipt_supported: true,
            execution_backends: vec![
                ProviderAdapterTrainingExecutionBackend::AppleFoundationModels,
            ],
            adapter_families: vec!["apple.foundation_models".to_string()],
            adapter_formats: vec!["apple.fmadapter".to_string()],
            validator_policy_refs: vec!["policy://validator/apple_adapter/helpdesk".to_string()],
            checkpoint_families: vec!["apple_adapter".to_string()],
            environment_refs: vec!["env.openagents.apple_adapter.helpdesk.core".to_string()],
            minimum_memory_gb: Some(24),
            available_memory_gb: Some(32),
            settlement_trigger: Some(
                ProviderAdapterTrainingSettlementTrigger::AcceptedContribution,
            ),
        }
    }

    fn authority_window() -> ComputeAdapterTrainingWindow {
        ComputeAdapterTrainingWindow {
            window_id: "adapter.window.alpha".to_string(),
            training_run_id: "train.apple_adapter.helpdesk.alpha".to_string(),
            stage_id: "sft".to_string(),
            contributor_set_revision_id: "contributors.rev.1".to_string(),
            validator_policy_ref: "policy://validator/apple_adapter/helpdesk".to_string(),
            adapter_target_id: "adapter.target.helpdesk".to_string(),
            adapter_family: "apple.foundation_models".to_string(),
            base_model_ref: "model://apple.foundation".to_string(),
            adapter_format: "apple.fmadapter".to_string(),
            source_policy_revision: ComputeAdapterPolicyRevision {
                policy_family: "policy://training/apple_adapter/helpdesk".to_string(),
                revision_id: "policy-rev-7".to_string(),
                revision_number: Some(7),
                policy_digest: "sha256:policy-7".to_string(),
                parent_revision_id: Some("policy-rev-6".to_string()),
                produced_at_ms: 1_762_000_000_000,
            },
            source_checkpoint_pointer:
                openagents_kernel_core::compute::ComputeAdapterCheckpointPointer {
                    scope_kind: "training_run".to_string(),
                    scope_id: "train.apple_adapter.helpdesk.alpha".to_string(),
                    checkpoint_family: "apple_adapter".to_string(),
                    checkpoint_ref: "checkpoint://apple_adapter/base".to_string(),
                    manifest_digest: "sha256:manifest-7".to_string(),
                    updated_at_ms: 1_762_000_000_010,
                    pointer_digest: "sha256:pointer-7".to_string(),
                },
            status: ComputeAdapterWindowStatus::Reconciled,
            total_contributions: 1,
            admitted_contributions: 1,
            accepted_contributions: 1,
            quarantined_contributions: 0,
            rejected_contributions: 0,
            replay_required_contributions: 0,
            replay_checked_contributions: 1,
            held_out_average_score_bps: Some(9_500),
            benchmark_pass_rate_bps: Some(9_400),
            runtime_smoke_passed: Some(true),
            promotion_ready: true,
            gate_reason_codes: Vec::new(),
            window_summary_digest: "sha256:window-summary".to_string(),
            promotion_disposition: Some(ComputeAdapterPromotionDisposition::Promoted),
            hold_reason_codes: Vec::new(),
            aggregated_delta_digest: Some("sha256:aggregate".to_string()),
            output_policy_revision: None,
            output_checkpoint_pointer: None,
            accepted_outcome_id: Some("accepted.training.apple.alpha".to_string()),
            recorded_at_ms: 1_762_000_000_020,
            metadata: serde_json::Value::Null,
        }
    }

    fn authority_contribution() -> ComputeAdapterContributionOutcome {
        ComputeAdapterContributionOutcome {
            contribution_id: "contrib.alpha".to_string(),
            training_run_id: "train.apple_adapter.helpdesk.alpha".to_string(),
            stage_id: "sft".to_string(),
            window_id: "adapter.window.alpha".to_string(),
            contributor_set_revision_id: "contributors.rev.1".to_string(),
            assignment_id: "assignment.alpha".to_string(),
            contributor_node_id: "node.alpha".to_string(),
            worker_id: "worker.alpha".to_string(),
            validator_policy_ref: "policy://validator/apple_adapter/helpdesk".to_string(),
            adapter_target_id: "adapter.target.helpdesk".to_string(),
            adapter_family: "apple.foundation_models".to_string(),
            base_model_ref: "model://apple.foundation".to_string(),
            adapter_format: "apple.fmadapter".to_string(),
            dataset_slice: ComputeAdapterDatasetSlice {
                dataset_id: "dataset://apple_adapter/helpdesk/train".to_string(),
                split_name: "train".to_string(),
                slice_id: "slice.alpha".to_string(),
                slice_digest: "sha256:slice-alpha".to_string(),
            },
            source_policy_revision: ComputeAdapterPolicyRevision {
                policy_family: "policy://training/apple_adapter/helpdesk".to_string(),
                revision_id: "policy-rev-7".to_string(),
                revision_number: Some(7),
                policy_digest: "sha256:policy-7".to_string(),
                parent_revision_id: Some("policy-rev-6".to_string()),
                produced_at_ms: 1_762_000_000_000,
            },
            source_checkpoint_pointer:
                openagents_kernel_core::compute::ComputeAdapterCheckpointPointer {
                    scope_kind: "training_run".to_string(),
                    scope_id: "train.apple_adapter.helpdesk.alpha".to_string(),
                    checkpoint_family: "apple_adapter".to_string(),
                    checkpoint_ref: "checkpoint://apple_adapter/base".to_string(),
                    manifest_digest: "sha256:manifest-7".to_string(),
                    updated_at_ms: 1_762_000_000_010,
                    pointer_digest: "sha256:pointer-7".to_string(),
                },
            submission_receipt_digest: "sha256:submission".to_string(),
            artifact_id: "artifact.alpha".to_string(),
            manifest_digest: "sha256:manifest-alpha".to_string(),
            object_digest: "sha256:object-alpha".to_string(),
            artifact_receipt_digest: "sha256:artifact-receipt".to_string(),
            provenance_bundle_digest: "sha256:provenance".to_string(),
            security_receipt_digest: "sha256:security".to_string(),
            replay_receipt_digest: Some("sha256:replay".to_string()),
            validator_disposition: ComputeAdapterContributionDisposition::Accepted,
            validation_reason_codes: Vec::new(),
            validator_receipt_digest: "sha256:validator".to_string(),
            aggregation_eligibility: ComputeAdapterAggregationEligibility::Eligible,
            accepted_for_aggregation: true,
            aggregation_weight_bps: Some(10_000),
            promotion_receipt_digest: Some("sha256:promotion".to_string()),
            recorded_at_ms: 1_762_000_000_020,
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn apple_backend_wins_when_both_backends_are_ready() {
        let availability = ProviderAvailability {
            local_gemma: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ready_health(None, "apple-foundation-model", None),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };

        assert_eq!(
            availability.active_inference_backend(),
            Some(ProviderBackendKind::AppleFoundationModels)
        );
        assert_eq!(
            availability.execution_backend_label(),
            "Apple Foundation Models bridge"
        );
    }

    #[test]
    fn controls_gate_products_by_product_id() {
        let mut controls = ProviderInventoryControls::default();
        assert!(controls.is_product_advertised("psionic.local.inference.gemma.single_node"));
        assert!(!controls.is_product_advertised("psionic.local.embeddings.gemma.single_node"));
        assert!(
            controls.is_product_advertised("psionic.cluster.inference.pooled.remote_whole_request")
        );
        assert!(controls.is_product_advertised("psionic.cluster.inference.pooled.replicated"));
        assert!(controls.is_product_advertised("psionic.cluster.inference.pooled.dense_split"));
        assert!(controls.is_product_advertised("psionic.cluster.inference.pooled.sparse_expert"));
        assert!(
            controls.is_product_advertised(
                "psionic.local.inference.apple_foundation_models.single_node"
            )
        );
        assert!(controls.is_product_advertised(
            "psionic.local.adapter_hosting.apple_foundation_models.single_node"
        ));
        assert!(!controls.is_product_advertised(
            "psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated"
        ));

        let enabled = controls.toggle(ProviderComputeProduct::GptOssEmbeddings);
        assert!(!enabled);
        assert!(!controls.is_product_advertised("psionic.local.embeddings.gemma.single_node"));
    }

    #[test]
    fn local_gemma_product_ids_and_aliases_preserve_truthful_backend_identity() {
        let inference = ProviderComputeProduct::GptOssInference;
        let descriptor = inference.descriptor();

        assert_eq!(
            inference.product_id(),
            "psionic.local.inference.gemma.single_node"
        );
        assert_eq!(inference.backend_kind(), Some(ProviderBackendKind::GptOss));
        assert_eq!(inference.backend_label(), "local_gemma");
        assert_eq!(
            descriptor.product_id,
            "psionic.local.inference.gemma.single_node"
        );
        assert_eq!(descriptor.compute_family, "inference");
        assert_eq!(descriptor.backend_family, "local_gemma");

        assert_eq!(
            ProviderComputeProduct::for_product_id("gpt_oss.text_generation"),
            Some(ProviderComputeProduct::GptOssInference)
        );
        assert_eq!(
            ProviderComputeProduct::for_product_id("psionic.local.inference.gemma.single_node"),
            Some(ProviderComputeProduct::GptOssInference)
        );
        assert_eq!(
            ProviderComputeProduct::for_product_id("ollama.text_generation"),
            Some(ProviderComputeProduct::GptOssInference)
        );

        let legacy_descriptor =
            describe_provider_product_id("ollama.text_generation").expect("legacy alias");
        assert_eq!(
            legacy_descriptor.product_id,
            "psionic.local.inference.gemma.single_node"
        );
        assert_eq!(legacy_descriptor.backend_family, "local_gemma");
    }

    #[test]
    fn availability_serializes_local_gemma_and_omits_inert_legacy_branches() {
        let availability = ProviderAvailability {
            local_gemma: ready_health(Some("gemma4:e4b"), "gemma4:e4b", Some(140)),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };

        let value = serde_json::to_value(&availability).expect("serialize availability");

        assert!(value.get("local_gemma").is_some());
        assert!(value.get("gpt_oss").is_none());
        assert!(value.get("apple_foundation_models").is_none());
        assert!(value.get("apple_adapter_hosting").is_none());
        assert!(value.get("adapter_training_contributor").is_none());
        assert!(value.get("pooled_inference").is_none());
    }

    #[test]
    fn availability_accepts_legacy_gpt_oss_alias_on_decode() {
        let value = serde_json::json!({
            "gpt_oss": {
                "reachable": true,
                "ready": true,
                "configured_model": "gemma4:e4b",
                "ready_model": "gemma4:e4b",
                "available_models": ["gemma4:e4b"]
            },
            "sandbox": {
                "runtimes": [],
                "profiles": []
            }
        });

        let availability: ProviderAvailability =
            serde_json::from_value(value).expect("decode legacy alias");

        assert!(availability.local_gemma.ready);
        assert_eq!(
            availability.local_gemma.ready_model.as_deref(),
            Some("gemma4:e4b")
        );
    }

    #[test]
    fn inventory_row_serializes_target_as_canonical_product_id() {
        let row = ProviderInventoryRow {
            target: ProviderComputeProduct::GptOssInference,
            enabled: true,
            backend_ready: true,
            eligible: true,
            capability_summary: "backend=local_gemma".to_string(),
            market_receipt_class: "accepted_delivery".to_string(),
            earnings_summary: "ready".to_string(),
            source_badge: "pylon.serve".to_string(),
            capacity_lot_id: None,
            total_quantity: 1,
            reserved_quantity: 0,
            available_quantity: 1,
            delivery_state: "idle".to_string(),
            price_floor_sats: 21,
            terms_label: "spot".to_string(),
            forward_capacity_lot_id: None,
            forward_delivery_window_label: None,
            forward_total_quantity: 0,
            forward_reserved_quantity: 0,
            forward_available_quantity: 0,
            forward_terms_label: Some("forward".to_string()),
        };

        let value = serde_json::to_value(&row).expect("serialize inventory row");

        assert_eq!(
            value.get("target").and_then(|entry| entry.as_str()),
            Some("psionic.local.inference.gemma.single_node")
        );
    }

    #[test]
    fn inventory_row_accepts_legacy_enum_target_on_decode() {
        let value = serde_json::json!({
            "target": "gpt_oss_inference",
            "enabled": true,
            "backend_ready": true,
            "eligible": true,
            "capability_summary": "backend=local_gemma",
            "market_receipt_class": "accepted_delivery",
            "earnings_summary": "ready",
            "source_badge": "pylon.serve",
            "capacity_lot_id": null,
            "total_quantity": 1,
            "reserved_quantity": 0,
            "available_quantity": 1,
            "delivery_state": "idle",
            "price_floor_sats": 21,
            "terms_label": "spot",
            "forward_capacity_lot_id": null,
            "forward_delivery_window_label": null,
            "forward_total_quantity": 0,
            "forward_reserved_quantity": 0,
            "forward_available_quantity": 0,
            "forward_terms_label": "forward"
        });

        let decoded: ProviderInventoryRow =
            serde_json::from_value(value).expect("decode legacy inventory row");

        assert_eq!(decoded.target, ProviderComputeProduct::GptOssInference);
    }

    #[test]
    fn derive_provider_products_reflects_backend_health_and_capability_summary() {
        let availability = ProviderAvailability {
            local_gemma: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ProviderBackendHealth {
                reachable: true,
                ready: false,
                configured_model: None,
                ready_model: None,
                available_models: vec!["apple-foundation-model".to_string()],
                last_error: Some("apple fm unavailable".to_string()),
                last_action: Some("health check failed".to_string()),
                availability_message: Some("apple_intelligence_disabled".to_string()),
                latency_ms_p50: None,
            },
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let products =
            derive_provider_products(&availability, &ProviderInventoryControls::default());

        assert_eq!(products.len(), 3);
        assert!(products[0].eligible);
        assert!(!products[1].eligible);
        assert!(!products[2].eligible);
        assert!(
            products[0]
                .capability_summary
                .contains("latency_ms_p50=140")
        );
        assert!(
            products[1]
                .capability_summary
                .contains("platform_gate=apple_intelligence_disabled")
        );
        assert!(
            products[2]
                .capability_summary
                .contains("compatible_adapters=0")
        );
    }

    #[test]
    fn pooled_inference_products_track_remote_and_replicated_readiness() {
        let availability = ProviderAvailability {
            local_gemma: ProviderBackendHealth::default(),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability {
                available: true,
                source: "mesh_management".to_string(),
                management_base_url: Some("http://127.0.0.1:7878".to_string()),
                topology_digest: Some("mesh.topology.1".to_string()),
                default_model: Some("gemma4:e4b".to_string()),
                membership_state: "joined".to_string(),
                member_count: 2,
                warm_replica_count: 2,
                local_worker_id: Some("openai_compat".to_string()),
                local_serving_state: "proxying".to_string(),
                served_mesh_role: Some("worker".to_string()),
                served_mesh_posture: Some("thin_client".to_string()),
                execution_mode: Some("remote_whole_request".to_string()),
                execution_engine: Some("openai_compat".to_string()),
                fallback_posture: Some("thin_client_remote_only".to_string()),
                last_error: None,
                targetable_models: vec![
                    ProviderPooledInferenceTargetStatus {
                        model: "gemma4:e4b".to_string(),
                        family: "gemma4".to_string(),
                        supported_endpoints: vec![
                            "/v1/chat/completions".to_string(),
                            "/v1/responses".to_string(),
                        ],
                        structured_outputs: true,
                        tool_calling: true,
                        response_state: true,
                        warm_replica_count: 2,
                        local_warm_replica: false,
                        cluster_execution_modes: vec![
                            "remote_whole_request".to_string(),
                            "replicated".to_string(),
                            "dense_split".to_string(),
                        ],
                        cluster_execution_topologies: vec![
                            "replicated".to_string(),
                            "pipeline_sharded".to_string(),
                        ],
                        participating_workers: vec![
                            "openai_compat".to_string(),
                            "worker-gpu-a".to_string(),
                        ],
                    },
                    ProviderPooledInferenceTargetStatus {
                        model: "gpt-oss:20b".to_string(),
                        family: "gpt_oss".to_string(),
                        supported_endpoints: vec!["/v1/chat/completions".to_string()],
                        structured_outputs: true,
                        tool_calling: true,
                        response_state: false,
                        warm_replica_count: 1,
                        local_warm_replica: true,
                        cluster_execution_modes: vec!["remote_whole_request".to_string()],
                        cluster_execution_topologies: Vec::new(),
                        participating_workers: vec!["openai_compat".to_string()],
                    },
                    ProviderPooledInferenceTargetStatus {
                        model: "gemma4:26b".to_string(),
                        family: "gemma4".to_string(),
                        supported_endpoints: vec![
                            "/v1/chat/completions".to_string(),
                            "/v1/responses".to_string(),
                        ],
                        structured_outputs: false,
                        tool_calling: true,
                        response_state: true,
                        warm_replica_count: 1,
                        local_warm_replica: false,
                        cluster_execution_modes: vec!["sparse_expert".to_string()],
                        cluster_execution_topologies: vec!["tensor_sharded".to_string()],
                        participating_workers: vec![
                            "worker-gpu-a".to_string(),
                            "worker-gpu-b".to_string(),
                        ],
                    },
                ],
            },
            sandbox: ProviderSandboxAvailability::default(),
        };
        let products =
            derive_provider_products(&availability, &ProviderInventoryControls::default());
        let remote = products
            .iter()
            .find(|product| {
                product.product == ProviderComputeProduct::PooledInferenceRemoteWholeRequest
            })
            .expect("remote pooled product");
        let replicated = products
            .iter()
            .find(|product| {
                product.product == ProviderComputeProduct::PooledInferenceReplicatedServing
            })
            .expect("replicated pooled product");
        let dense_split = products
            .iter()
            .find(|product| product.product == ProviderComputeProduct::PooledInferenceDenseSplit)
            .expect("dense split pooled product");
        let sparse_expert = products
            .iter()
            .find(|product| product.product == ProviderComputeProduct::PooledInferenceSparseExpert)
            .expect("sparse expert pooled product");

        assert!(remote.backend_ready);
        assert!(remote.eligible);
        assert!(
            remote
                .capability_summary
                .contains("default_model=gemma4:e4b")
        );
        assert!(
            remote
                .capability_summary
                .contains("target_models=gemma4:e4b:gemma4:remote_whole_request+replicated+dense_split:replicated+pipeline_sharded:openai_compat+worker-gpu-a:2")
        );
        assert!(replicated.backend_ready);
        assert!(replicated.eligible);
        assert!(
            replicated
                .capability_summary
                .contains("mode_models=gemma4:e4b")
        );
        assert!(dense_split.backend_ready);
        assert!(dense_split.eligible);
        assert!(
            dense_split
                .capability_summary
                .contains("mode_topologies=pipeline_sharded")
        );
        assert!(sparse_expert.backend_ready);
        assert!(sparse_expert.eligible);
        assert!(
            sparse_expert
                .capability_summary
                .contains("mode_models=gemma4:26b")
        );
    }

    #[test]
    fn product_descriptor_preserves_launch_and_sandbox_taxonomy() {
        let embeddings = describe_provider_product_id("gpt_oss.embeddings");
        assert_eq!(
            embeddings
                .as_ref()
                .map(|descriptor| descriptor.compute_family.as_str()),
            Some("embeddings")
        );
        assert_eq!(
            embeddings
                .as_ref()
                .map(|descriptor| descriptor.backend_family.as_str()),
            Some("local_gemma")
        );
        assert_eq!(
            embeddings
                .as_ref()
                .and_then(|descriptor| descriptor.sandbox_execution_class.as_deref()),
            None
        );
        assert_eq!(
            embeddings
                .as_ref()
                .map(|descriptor| descriptor.product_id.as_str()),
            Some("psionic.local.embeddings.gemma.single_node")
        );

        let sandbox = describe_provider_product_id("sandbox.python.exec");
        assert_eq!(
            sandbox
                .as_ref()
                .map(|descriptor| descriptor.compute_family.as_str()),
            Some("sandbox_execution")
        );
        assert_eq!(
            sandbox
                .as_ref()
                .map(|descriptor| descriptor.backend_family.as_str()),
            Some("sandbox")
        );
        assert_eq!(
            sandbox
                .as_ref()
                .and_then(|descriptor| descriptor.sandbox_execution_class.as_deref()),
            Some("sandbox.python.exec")
        );
        assert_eq!(
            sandbox
                .as_ref()
                .map(|descriptor| descriptor.product_id.as_str()),
            Some("psionic.remote_sandbox.sandbox_execution.python_exec.sandbox_isolated")
        );

        let adapter_hosting =
            describe_provider_product_id("apple_foundation_models.adapter_hosting");
        assert_eq!(
            adapter_hosting
                .as_ref()
                .map(|descriptor| descriptor.compute_family.as_str()),
            Some("adapter_hosting")
        );
        assert_eq!(
            adapter_hosting
                .as_ref()
                .map(|descriptor| descriptor.backend_family.as_str()),
            Some("apple_foundation_models")
        );
        assert_eq!(
            adapter_hosting
                .as_ref()
                .map(|descriptor| descriptor.product_id.as_str()),
            Some("psionic.local.adapter_hosting.apple_foundation_models.single_node")
        );

        let adapter_training = describe_provider_product_id("adapter_training.contributor");
        assert_eq!(
            adapter_training
                .as_ref()
                .map(|descriptor| descriptor.compute_family.as_str()),
            Some("training")
        );
        assert_eq!(
            adapter_training
                .as_ref()
                .map(|descriptor| descriptor.backend_family.as_str()),
            Some("psionic_train")
        );
        assert_eq!(
            adapter_training
                .as_ref()
                .map(|descriptor| descriptor.product_id.as_str()),
            Some("psionic.cluster.training.adapter_contributor.cluster_attached")
        );

        let pooled_remote =
            describe_provider_product_id("gpt_oss.clustered_inference.remote_whole_request");
        assert_eq!(
            pooled_remote
                .as_ref()
                .map(|descriptor| descriptor.compute_family.as_str()),
            Some("inference")
        );
        assert_eq!(
            pooled_remote
                .as_ref()
                .map(|descriptor| descriptor.backend_family.as_str()),
            Some("pooled_inference")
        );
        assert_eq!(
            pooled_remote
                .as_ref()
                .map(|descriptor| descriptor.product_id.as_str()),
            Some("psionic.cluster.inference.pooled.remote_whole_request")
        );
    }

    #[test]
    fn derive_provider_products_includes_declared_sandbox_profiles_when_enabled()
    -> Result<(), Box<dyn std::error::Error>> {
        let availability = ProviderAvailability {
            local_gemma: ProviderBackendHealth::default(),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: detect_sandbox_supply(&ProviderSandboxDetectionConfig {
                path_entries: Vec::new(),
                declared_profiles: vec![ProviderSandboxProfileSpec {
                    profile_id: "python-batch".to_string(),
                    execution_class: ProviderSandboxExecutionClass::PythonExec,
                    runtime_family: "python3".to_string(),
                    runtime_version: Some("3.11".to_string()),
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
                }],
            }),
        };
        let mut controls = ProviderInventoryControls::default();
        controls.sandbox_python_exec_enabled = true;

        let products = derive_provider_products(&availability, &controls);
        let sandbox_product = products
            .iter()
            .find(|product| product.product == ProviderComputeProduct::SandboxPythonExec)
            .ok_or_else(|| std::io::Error::other("missing sandbox python product"))?;
        ensure(
            !sandbox_product.backend_ready,
            "sandbox product should stay unready without a detected runtime",
        )?;
        ensure(
            sandbox_product
                .capability_summary
                .contains("sandbox.python.exec"),
            "sandbox capability summary should include python execution class",
        )?;
        ensure(
            sandbox_product
                .capability_summary
                .contains("profile_ids=python-batch"),
            "sandbox capability summary should include declared profile id",
        )?;
        Ok(())
    }

    #[test]
    fn apple_adapter_hosting_requires_runtime_inventory_and_compatible_adapter() {
        let availability = ProviderAvailability {
            local_gemma: ProviderBackendHealth::default(),
            apple_foundation_models: ready_health(None, "apple-foundation-model", Some(38)),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability {
                inventory_supported: true,
                attach_supported: true,
                adapters: vec![
                    ProviderAppleAdapterHostingEntry {
                        adapter_id: "helpdesk".to_string(),
                        package_digest: Some("sha256:helpdesk".to_string()),
                        compatible: true,
                        compatibility_message: Some(
                            "compatible with the current Apple FM runtime".to_string(),
                        ),
                        ..ProviderAppleAdapterHostingEntry::default()
                    },
                    ProviderAppleAdapterHostingEntry {
                        adapter_id: "legacy".to_string(),
                        package_digest: Some("sha256:legacy".to_string()),
                        compatible: false,
                        compatibility_reason_code: Some("base_model_mismatch".to_string()),
                        ..ProviderAppleAdapterHostingEntry::default()
                    },
                ],
            },
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let products =
            derive_provider_products(&availability, &ProviderInventoryControls::default());
        let adapter_hosting = products
            .iter()
            .find(|product| {
                product.product == ProviderComputeProduct::AppleFoundationModelsAdapterHosting
            })
            .expect("adapter hosting product");

        assert!(adapter_hosting.backend_ready);
        assert!(adapter_hosting.eligible);
        assert!(
            adapter_hosting
                .capability_summary
                .contains("compatible_adapters=1")
        );
        assert!(
            adapter_hosting
                .capability_summary
                .contains("compatible_adapter_digests=sha256:helpdesk")
        );
    }

    #[test]
    fn apple_adapter_hosting_stays_blocked_without_compatible_loaded_adapter() {
        let availability = ProviderAvailability {
            local_gemma: ProviderBackendHealth::default(),
            apple_foundation_models: ready_health(None, "apple-foundation-model", Some(38)),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability {
                inventory_supported: true,
                attach_supported: true,
                adapters: vec![ProviderAppleAdapterHostingEntry {
                    adapter_id: "legacy".to_string(),
                    package_digest: Some("sha256:legacy".to_string()),
                    compatible: false,
                    compatibility_reason_code: Some("base_model_mismatch".to_string()),
                    ..ProviderAppleAdapterHostingEntry::default()
                }],
            },
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let products =
            derive_provider_products(&availability, &ProviderInventoryControls::default());
        let adapter_hosting = products
            .iter()
            .find(|product| {
                product.product == ProviderComputeProduct::AppleFoundationModelsAdapterHosting
            })
            .expect("adapter hosting product");

        assert!(!adapter_hosting.backend_ready);
        assert!(!adapter_hosting.eligible);
        assert!(
            adapter_hosting
                .capability_summary
                .contains("compatible_adapters=0")
        );
    }

    #[test]
    fn adapter_training_contributor_product_derives_from_availability() {
        let availability = ProviderAvailability {
            local_gemma: ProviderBackendHealth::default(),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ready_adapter_training_contributor(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let mut controls = ProviderInventoryControls::default();
        controls.adapter_training_contributor_enabled = true;

        let products = derive_provider_products(&availability, &controls);
        let contributor = products
            .iter()
            .find(|product| product.product == ProviderComputeProduct::AdapterTrainingContributor)
            .expect("adapter training contributor product");

        assert!(contributor.backend_ready);
        assert!(contributor.eligible);
        assert!(
            contributor
                .capability_summary
                .contains("execution_backends=apple_foundation_models")
        );
        assert!(
            contributor
                .capability_summary
                .contains("settlement_trigger=accepted_contribution")
        );
    }

    #[test]
    fn adapter_training_match_verdict_reports_missing_capabilities() {
        let availability = ProviderAdapterTrainingContributorAvailability {
            contributor_supported: true,
            coordinator_match_supported: false,
            authority_receipt_supported: true,
            execution_backends: vec![
                ProviderAdapterTrainingExecutionBackend::AppleFoundationModels,
            ],
            adapter_families: vec!["apple.foundation_models".to_string()],
            adapter_formats: vec!["apple.fmadapter".to_string()],
            validator_policy_refs: vec!["policy://validator/apple_adapter/helpdesk".to_string()],
            checkpoint_families: vec!["apple_adapter".to_string()],
            environment_refs: vec!["env.openagents.apple_adapter.helpdesk.core".to_string()],
            minimum_memory_gb: Some(24),
            available_memory_gb: Some(16),
            settlement_trigger: Some(
                ProviderAdapterTrainingSettlementTrigger::AcceptedContribution,
            ),
        };
        let verdict = match_adapter_training_contributor(
            &availability,
            &ProviderAdapterTrainingMatchRequest {
                training_run_id: "train.apple_adapter.helpdesk.alpha".to_string(),
                adapter_family: "apple.foundation_models".to_string(),
                adapter_format: "apple.fmadapter".to_string(),
                validator_policy_ref: "policy://validator/apple_adapter/helpdesk".to_string(),
                environment_ref: Some("env.openagents.apple_adapter.helpdesk.core".to_string()),
                checkpoint_family: Some("apple_adapter".to_string()),
                minimum_memory_gb: Some(24),
                execution_backend: Some(
                    ProviderAdapterTrainingExecutionBackend::AppleFoundationModels,
                ),
                settlement_trigger: ProviderAdapterTrainingSettlementTrigger::AcceptedContribution,
            },
        );
        assert!(!verdict.eligible);
        assert_eq!(
            verdict.reason_codes,
            vec![
                ProviderAdapterTrainingMatchReasonCode::CoordinatorMatchUnavailable,
                ProviderAdapterTrainingMatchReasonCode::MemoryInsufficient,
            ]
        );
    }

    #[test]
    fn settlement_hook_derives_from_authority_window_and_contribution() {
        let contribution_hook = settlement_hook_from_authority(
            &authority_window(),
            Some(&authority_contribution()),
            ProviderAdapterTrainingSettlementTrigger::AcceptedContribution,
        )
        .expect("accepted contribution settlement hook");
        assert_eq!(
            contribution_hook.contribution_id.as_deref(),
            Some("contrib.alpha")
        );
        assert_eq!(
            contribution_hook.validator_receipt_digest.as_deref(),
            Some("sha256:validator")
        );

        let window_hook = settlement_hook_from_authority(
            &authority_window(),
            None,
            ProviderAdapterTrainingSettlementTrigger::AcceptedSealedWindow,
        )
        .expect("accepted sealed window settlement hook");
        assert_eq!(
            window_hook.accepted_outcome_id.as_deref(),
            Some("accepted.training.apple.alpha")
        );
        assert!(window_hook.contribution_id.is_none());
    }

    #[test]
    fn lifecycle_degrades_on_relay_error() {
        let availability = ProviderAvailability::default();
        let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
            current_mode: ProviderMode::Connecting,
            ingress_mode: ProviderIngressMode::Online,
            relay_error: Some("relay write failed"),
            availability: &availability,
            backend_unavailable_detail: None,
        });

        assert_eq!(
            transition,
            ProviderLifecycleTransition::Degraded {
                reason_code: "NIP90_RELAY_INGRESS_ERROR",
                error_detail: "relay write failed".to_string(),
                failure_class: ProviderFailureClass::Relay,
            }
        );
    }

    #[test]
    fn lifecycle_degrades_when_no_backend_is_ready() {
        let availability = ProviderAvailability::default();
        let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
            current_mode: ProviderMode::Connecting,
            ingress_mode: ProviderIngressMode::Online,
            relay_error: None,
            availability: &availability,
            backend_unavailable_detail: Some("No local inference backend is ready"),
        });

        assert_eq!(
            transition,
            ProviderLifecycleTransition::Degraded {
                reason_code: "INFERENCE_BACKEND_UNAVAILABLE",
                error_detail: "No local inference backend is ready".to_string(),
                failure_class: ProviderFailureClass::Execution,
            }
        );
    }

    #[test]
    fn lifecycle_holds_while_offline_even_if_backends_are_ready() {
        let availability = ProviderAvailability {
            local_gemma: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
            current_mode: ProviderMode::Offline,
            ingress_mode: ProviderIngressMode::Online,
            relay_error: None,
            availability: &availability,
            backend_unavailable_detail: None,
        });

        assert_eq!(transition, ProviderLifecycleTransition::HoldCurrent);
    }

    #[test]
    fn lifecycle_promotes_non_offline_runtime_to_online_when_backend_is_ready() {
        let availability = ProviderAvailability {
            local_gemma: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
            pooled_inference: ProviderPooledInferenceAvailability::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
            current_mode: ProviderMode::Connecting,
            ingress_mode: ProviderIngressMode::Online,
            relay_error: None,
            availability: &availability,
            backend_unavailable_detail: None,
        });

        assert_eq!(
            transition,
            ProviderLifecycleTransition::Online {
                active_backend: ProviderBackendKind::GptOss,
            }
        );
    }
}
