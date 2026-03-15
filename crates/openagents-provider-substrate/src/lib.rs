//! Narrow shared provider substrate for reusable provider runtime semantics.
//!
//! This crate intentionally owns only product-agnostic provider domain logic:
//! backend health, launch product derivation, inventory controls/models, and
//! provider lifecycle derivation. App-specific UX, execution snapshots, and
//! orchestration stay in app crates.

mod admin;
mod sandbox;
mod sandbox_execution;

pub use admin::*;
pub use sandbox::*;
pub use sandbox_execution::*;

use openagents_kernel_core::compute::{
    ComputeAdapterAggregationEligibility, ComputeAdapterContributionDisposition,
    ComputeAdapterContributionOutcome, ComputeAdapterTrainingWindow,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBackendKind {
    #[serde(alias = "ollama")]
    GptOss,
    AppleFoundationModels,
    PsionicTrain,
}

impl ProviderBackendKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GptOss => "local GPT-OSS runtime",
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
            Self::GptOssUnavailable => "GPT_OSS_UNAVAILABLE",
            Self::GptOssModelUnavailable => "GPT_OSS_MODEL_UNAVAILABLE",
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
            Self::GptOssUnavailable => "Local GPT-OSS backend is unavailable",
            Self::GptOssModelUnavailable => "No local GPT-OSS serving model is ready",
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
    #[serde(alias = "ollama")]
    pub gpt_oss: ProviderBackendHealth,
    pub apple_foundation_models: ProviderBackendHealth,
    #[serde(default)]
    pub apple_adapter_hosting: ProviderAppleAdapterHostingAvailability,
    #[serde(default)]
    pub adapter_training_contributor: ProviderAdapterTrainingContributorAvailability,
    pub sandbox: ProviderSandboxAvailability,
}

impl ProviderAvailability {
    pub fn active_inference_backend(&self) -> Option<ProviderBackendKind> {
        if self.apple_foundation_models.is_ready() {
            Some(ProviderBackendKind::AppleFoundationModels)
        } else if self.gpt_oss.is_ready() {
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
            ProviderComputeProduct::GptOssInference => self.gpt_oss.is_ready(),
            ProviderComputeProduct::GptOssEmbeddings => false,
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
            | ProviderComputeProduct::AppleFoundationModelsInference
            | ProviderComputeProduct::AppleFoundationModelsAdapterHosting
            | ProviderComputeProduct::AdapterTrainingContributor => {
                product.capability_summary(self)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderComputeProduct {
    GptOssInference,
    GptOssEmbeddings,
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
    pub const fn all() -> [Self; 8] {
        [
            Self::GptOssInference,
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
            Self::GptOssInference => "psionic.local.inference.gpt_oss.single_node",
            Self::GptOssEmbeddings => "psionic.local.embeddings.gpt_oss.single_node",
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
            Self::GptOssInference => "GPT-OSS inference",
            Self::GptOssEmbeddings => "GPT-OSS embeddings",
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
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => None,
        }
    }

    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => "gpt_oss",
            Self::AppleFoundationModelsInference | Self::AppleFoundationModelsAdapterHosting => {
                "apple_foundation_models"
            }
            Self::AdapterTrainingContributor => "psionic_train",
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "sandbox",
        }
    }

    pub const fn compute_family_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::AppleFoundationModelsInference => "inference",
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
            | Self::AppleFoundationModelsInference
            | Self::AppleFoundationModelsAdapterHosting
            | Self::AdapterTrainingContributor => None,
        }
    }

    pub const fn capability_summary_base(self) -> &'static str {
        match self {
            Self::GptOssInference => "backend=gpt_oss execution=local_inference family=inference",
            Self::GptOssEmbeddings => {
                "backend=gpt_oss execution=local_inference family=embeddings status=unsupported"
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
                let health = &availability.gpt_oss;
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
            "psionic.local.inference.gpt_oss.single_node"
            | "ollama.text_generation"
            | "gpt_oss.text_generation" => Some(Self::GptOssInference),
            "psionic.local.embeddings.gpt_oss.single_node"
            | "ollama.embeddings"
            | "gpt_oss.embeddings" => Some(Self::GptOssEmbeddings),
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
}

pub fn describe_provider_product_id(product_id: &str) -> Option<ProviderProductDescriptor> {
    ProviderComputeProduct::for_product_id(product_id).map(ProviderComputeProduct::descriptor)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderInventoryRow {
    pub target: ProviderComputeProduct,
    pub enabled: bool,
    pub backend_ready: bool,
    pub eligible: bool,
    pub capability_summary: String,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderInventoryControls {
    #[serde(alias = "ollama_inference_enabled")]
    pub gpt_oss_inference_enabled: bool,
    #[serde(alias = "ollama_embeddings_enabled")]
    pub gpt_oss_embeddings_enabled: bool,
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
            gpt_oss_inference_enabled: true,
            gpt_oss_embeddings_enabled: false,
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
            ProviderComputeProduct::GptOssInference => self.gpt_oss_inference_enabled,
            ProviderComputeProduct::GptOssEmbeddings => false,
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
            ProviderComputeProduct::GptOssInference => &mut self.gpt_oss_inference_enabled,
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
                self.gpt_oss_embeddings_enabled = false;
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
            ProviderAdvertisedProduct {
                product,
                enabled,
                backend_ready,
                eligible: enabled && backend_ready,
                capability_summary: availability.capability_summary_for_product(product),
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
        ProviderInventoryControls, ProviderLifecycleInput, ProviderLifecycleTransition,
        ProviderMode, ProviderSandboxAvailability, ProviderSandboxDetectionConfig,
        ProviderSandboxExecutionClass, ProviderSandboxProfileSpec, derive_provider_lifecycle,
        derive_provider_products, describe_provider_product_id, detect_sandbox_supply,
        match_adapter_training_contributor, settlement_hook_from_authority,
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
            gpt_oss: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ready_health(None, "apple-foundation-model", None),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
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
        assert!(controls.is_product_advertised("psionic.local.inference.gpt_oss.single_node"));
        assert!(!controls.is_product_advertised("psionic.local.embeddings.gpt_oss.single_node"));
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
        assert!(!controls.is_product_advertised("psionic.local.embeddings.gpt_oss.single_node"));
    }

    #[test]
    fn gpt_oss_product_ids_and_aliases_preserve_truthful_backend_identity() {
        let inference = ProviderComputeProduct::GptOssInference;
        let descriptor = inference.descriptor();

        assert_eq!(
            inference.product_id(),
            "psionic.local.inference.gpt_oss.single_node"
        );
        assert_eq!(inference.backend_kind(), Some(ProviderBackendKind::GptOss));
        assert_eq!(inference.backend_label(), "gpt_oss");
        assert_eq!(
            descriptor.product_id,
            "psionic.local.inference.gpt_oss.single_node"
        );
        assert_eq!(descriptor.compute_family, "inference");
        assert_eq!(descriptor.backend_family, "gpt_oss");

        assert_eq!(
            ProviderComputeProduct::for_product_id("gpt_oss.text_generation"),
            Some(ProviderComputeProduct::GptOssInference)
        );
        assert_eq!(
            ProviderComputeProduct::for_product_id("psionic.local.inference.gpt_oss.single_node"),
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
            "psionic.local.inference.gpt_oss.single_node"
        );
        assert_eq!(legacy_descriptor.backend_family, "gpt_oss");
    }

    #[test]
    fn derive_provider_products_reflects_backend_health_and_capability_summary() {
        let availability = ProviderAvailability {
            gpt_oss: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
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
            Some("gpt_oss")
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
            Some("psionic.local.embeddings.gpt_oss.single_node")
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
    }

    #[test]
    fn derive_provider_products_includes_declared_sandbox_profiles_when_enabled()
    -> Result<(), Box<dyn std::error::Error>> {
        let availability = ProviderAvailability {
            gpt_oss: ProviderBackendHealth::default(),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
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
            gpt_oss: ProviderBackendHealth::default(),
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
            gpt_oss: ProviderBackendHealth::default(),
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
            gpt_oss: ProviderBackendHealth::default(),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ready_adapter_training_contributor(),
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
            gpt_oss: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
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
            gpt_oss: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ProviderBackendHealth::default(),
            apple_adapter_hosting: ProviderAppleAdapterHostingAvailability::default(),
            adapter_training_contributor: ProviderAdapterTrainingContributorAvailability::default(),
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
