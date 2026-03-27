//! Runtime-facing provider state for the embedded OpenAgents Runtime.
//!
//! This module keeps app-owned execution snapshots and UX-facing state local to
//! `autopilot-desktop`, while reusing the narrow shared provider substrate for
//! backend identity, launch-product derivation, inventory controls, and
//! provider lifecycle semantics.

use std::time::{Duration, Instant};

use crate::local_inference_runtime::LocalInferenceExecutionMetrics;
pub use openagents_provider_substrate::{
    ProviderAdapterTrainingContributorAvailability, ProviderAdapterTrainingExecutionBackend,
    ProviderAdapterTrainingSettlementTrigger, ProviderAdvertisedProduct,
    ProviderAppleAdapterHostingAvailability, ProviderAppleAdapterHostingEntry,
    ProviderAvailability, ProviderBackendHealth, ProviderBackendKind, ProviderBlocker,
    ProviderComputeProduct, ProviderFailureClass, ProviderInventoryControls, ProviderInventoryRow,
    ProviderMode, ProviderSandboxAvailability, ProviderSandboxDetectionConfig,
    derive_provider_products, detect_sandbox_supply,
};
use psionic_apple_fm::{
    AppleFmAdapterInventoryEntry, AppleFmSystemLanguageModel, AppleFmSystemLanguageModelGuardrails,
    AppleFmSystemLanguageModelUnavailableReason, AppleFmSystemLanguageModelUseCase,
};

pub type LocalInferenceBackend = ProviderBackendKind;
pub type EarnFailureClass = ProviderFailureClass;
pub type ProviderInventoryProductToggleTarget = ProviderComputeProduct;

const APPLE_ADAPTER_REFERENCE_ENVIRONMENT_REF: &str = "env.openagents.apple_adapter.helpdesk.core";
const APPLE_ADAPTER_REFERENCE_VALIDATOR_POLICY_REF: &str =
    "policy://validator/apple_adapter/helpdesk";
const APPLE_ADAPTER_REFERENCE_CHECKPOINT_FAMILY: &str = "apple_adapter";
const APPLE_ADAPTER_REFERENCE_ADAPTER_FAMILY: &str = "apple_adapter";
const APPLE_ADAPTER_REFERENCE_ADAPTER_FORMAT: &str = "openagents.apple-fmadapter.v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderIngressPolicyFilterKind {
    UnsupportedKind,
    TargetMismatch,
}

pub(crate) fn classify_provider_ingress_policy_filter_reason(
    reason: &str,
) -> Option<ProviderIngressPolicyFilterKind> {
    let normalized = reason.trim().to_ascii_lowercase();
    if normalized.contains("unsupported request kind") {
        Some(ProviderIngressPolicyFilterKind::UnsupportedKind)
    } else if normalized.contains("target policy mismatch") {
        Some(ProviderIngressPolicyFilterKind::TargetMismatch)
    } else {
        None
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderIngressPolicyFilterSummaryState {
    pub unsupported_kind_count: u64,
    pub target_mismatch_count: u64,
}

impl ProviderIngressPolicyFilterSummaryState {
    pub const fn is_empty(&self) -> bool {
        self.unsupported_kind_count == 0 && self.target_mismatch_count == 0
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_reason(&mut self, reason: &str) -> Option<ProviderIngressPolicyFilterKind> {
        let kind = classify_provider_ingress_policy_filter_reason(reason)?;
        match kind {
            ProviderIngressPolicyFilterKind::UnsupportedKind => {
                self.unsupported_kind_count = self.unsupported_kind_count.saturating_add(1);
            }
            ProviderIngressPolicyFilterKind::TargetMismatch => {
                self.target_mismatch_count = self.target_mismatch_count.saturating_add(1);
            }
        }
        Some(kind)
    }

    pub fn provider_status_line(&self) -> Option<String> {
        (!self.is_empty()).then(|| {
            format!(
                "ingress policy filtered expected relay traffic ({})",
                self.counts_fragment()
            )
        })
    }

    pub fn inbox_status_line(&self) -> Option<String> {
        (!self.is_empty()).then(|| {
            format!(
                "Policy-filtered live NIP-90 traffic is visible in Activity diagnostics ({})",
                self.counts_fragment()
            )
        })
    }

    fn counts_fragment(&self) -> String {
        let mut parts = Vec::new();
        if self.unsupported_kind_count > 0 {
            parts.push(format!("unsupported_kind={}", self.unsupported_kind_count));
        }
        if self.target_mismatch_count > 0 {
            parts.push(format!("target_mismatch={}", self.target_mismatch_count));
        }
        if parts.is_empty() {
            "none".to_string()
        } else {
            parts.join(", ")
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProviderGptOssRuntimeState {
    pub reachable: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub loaded_models: Vec<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_request_id: Option<String>,
    pub last_metrics: Option<LocalInferenceExecutionMetrics>,
    pub refreshed_at: Option<Instant>,
}

impl ProviderGptOssRuntimeState {
    pub fn is_ready(&self) -> bool {
        self.reachable && self.ready_model.is_some()
    }

    pub fn substrate_health(&self) -> ProviderBackendHealth {
        ProviderBackendHealth {
            reachable: self.reachable,
            ready: self.is_ready(),
            configured_model: self.configured_model.clone(),
            ready_model: self.ready_model.clone(),
            available_models: self.available_models.clone(),
            last_error: self.last_error.clone(),
            last_action: self.last_action.clone(),
            availability_message: None,
            latency_ms_p50: self
                .last_metrics
                .as_ref()
                .and_then(|metrics| metrics.total_duration_ns)
                .map(|duration_ns| duration_ns / 1_000_000),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProviderAppleFmRuntimeState {
    pub reachable: bool,
    pub model_available: bool,
    pub system_model: AppleFmSystemLanguageModel,
    pub unavailable_reason: Option<AppleFmSystemLanguageModelUnavailableReason>,
    pub supported_use_cases: Vec<AppleFmSystemLanguageModelUseCase>,
    pub supported_guardrails: Vec<AppleFmSystemLanguageModelGuardrails>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub adapter_inventory_supported: bool,
    pub adapter_attach_supported: bool,
    pub loaded_adapters: Vec<AppleFmAdapterInventoryEntry>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_request_id: Option<String>,
    pub last_metrics: Option<LocalInferenceExecutionMetrics>,
    pub refreshed_at: Option<Instant>,
    pub availability_message: Option<String>,
    pub bridge_status: Option<String>,
}

impl ProviderAppleFmRuntimeState {
    pub fn is_ready(&self) -> bool {
        self.reachable && self.model_available && self.ready_model.is_some()
    }

    pub fn has_authoritative_capability_state(&self) -> bool {
        self.is_ready()
            || self.availability_error_message().is_some()
            || matches!(self.bridge_status.as_deref(), Some("failed"))
    }

    pub fn availability_error_message(&self) -> Option<String> {
        self.last_error.clone().or_else(|| {
            (!self.model_available)
                .then(|| {
                    self.availability_message
                        .clone()
                        .filter(|message| !is_positive_apple_fm_availability_message(message))
                        .map(|message| humanize_apple_fm_availability_message(message.as_str()))
                        .or_else(|| {
                            self.unavailable_reason
                                .map(humanize_apple_fm_unavailable_reason)
                        })
                })
                .flatten()
        })
    }

    pub fn readiness_block_reason(&self) -> Option<String> {
        if self.is_ready() {
            return None;
        }
        self.availability_error_message().or_else(|| {
            match self.bridge_status.as_deref() {
                Some("starting") => {
                    Some("Apple Foundation Models bridge is starting.".to_string())
                }
                Some("failed") => Some(
                    "Apple Foundation Models bridge failed to start. Build and launch `swift/foundation-bridge`.".to_string(),
                ),
                _ if self.reachable => Some(
                    "Apple Foundation Models bridge reachable; waiting for model inventory."
                        .to_string(),
                ),
                _ => Some(
                    "Apple Foundation Models bridge is not running. Build and launch `swift/foundation-bridge`.".to_string(),
                ),
            }
        })
    }

    pub fn substrate_health(&self) -> ProviderBackendHealth {
        ProviderBackendHealth {
            reachable: self.reachable,
            ready: self.is_ready(),
            configured_model: None,
            ready_model: self.ready_model.clone(),
            available_models: self.available_models.clone(),
            last_error: self.last_error.clone(),
            last_action: self.last_action.clone(),
            availability_message: self.availability_message.clone().or_else(|| {
                self.unavailable_reason
                    .map(|reason| format!("Apple FM unavailable: {}", reason.label()))
            }),
            latency_ms_p50: self
                .last_metrics
                .as_ref()
                .and_then(|metrics| metrics.total_duration_ns)
                .map(|duration_ns| duration_ns / 1_000_000),
        }
    }

    pub fn substrate_adapter_hosting(&self) -> ProviderAppleAdapterHostingAvailability {
        ProviderAppleAdapterHostingAvailability {
            inventory_supported: self.adapter_inventory_supported,
            attach_supported: self.adapter_attach_supported,
            adapters: self
                .loaded_adapters
                .iter()
                .map(|entry| ProviderAppleAdapterHostingEntry {
                    adapter_id: entry.adapter.adapter_id.clone(),
                    package_digest: entry.adapter.package_digest.clone(),
                    base_model_signature: entry.base_model_signature.clone(),
                    package_format_version: entry.package_format_version.clone(),
                    draft_model_present: entry.draft_model_present,
                    compatible: entry.compatibility.compatible,
                    compatibility_reason_code: entry.compatibility.reason_code.clone(),
                    compatibility_message: entry.compatibility.message.clone(),
                    attached_session_count: entry.attached_session_ids.len(),
                })
                .collect(),
        }
    }

    pub fn substrate_training_contributor(&self) -> ProviderAdapterTrainingContributorAvailability {
        let contributor_supported = self.reachable || self.has_authoritative_capability_state();
        let runtime_ready = self.is_ready();
        ProviderAdapterTrainingContributorAvailability {
            contributor_supported,
            coordinator_match_supported: runtime_ready,
            authority_receipt_supported: runtime_ready,
            execution_backends: contributor_supported
                .then_some(ProviderAdapterTrainingExecutionBackend::AppleFoundationModels)
                .into_iter()
                .collect(),
            adapter_families: contributor_supported
                .then_some(APPLE_ADAPTER_REFERENCE_ADAPTER_FAMILY.to_string())
                .into_iter()
                .collect(),
            adapter_formats: contributor_supported
                .then_some(APPLE_ADAPTER_REFERENCE_ADAPTER_FORMAT.to_string())
                .into_iter()
                .collect(),
            validator_policy_refs: contributor_supported
                .then_some(APPLE_ADAPTER_REFERENCE_VALIDATOR_POLICY_REF.to_string())
                .into_iter()
                .collect(),
            checkpoint_families: contributor_supported
                .then_some(APPLE_ADAPTER_REFERENCE_CHECKPOINT_FAMILY.to_string())
                .into_iter()
                .collect(),
            environment_refs: contributor_supported
                .then_some(APPLE_ADAPTER_REFERENCE_ENVIRONMENT_REF.to_string())
                .into_iter()
                .collect(),
            minimum_memory_gb: None,
            available_memory_gb: None,
            settlement_trigger: contributor_supported
                .then_some(ProviderAdapterTrainingSettlementTrigger::AcceptedContribution),
        }
    }
}

fn is_positive_apple_fm_availability_message(message: &str) -> bool {
    message
        .trim()
        .eq_ignore_ascii_case("Foundation Models is available")
}

fn humanize_apple_fm_unavailable_reason(
    reason: AppleFmSystemLanguageModelUnavailableReason,
) -> String {
    match reason {
        AppleFmSystemLanguageModelUnavailableReason::AppleIntelligenceNotEnabled => {
            "Apple Intelligence is disabled. Enable it in System Settings > Apple Intelligence."
                .to_string()
        }
        AppleFmSystemLanguageModelUnavailableReason::DeviceNotEligible => {
            "This Mac is not eligible for Apple Foundation Models.".to_string()
        }
        AppleFmSystemLanguageModelUnavailableReason::ModelNotReady => {
            "Apple Foundation Models is still downloading or preparing the system model."
                .to_string()
        }
        AppleFmSystemLanguageModelUnavailableReason::Unknown => {
            "Apple Foundation Models is unavailable.".to_string()
        }
    }
}

fn humanize_apple_fm_availability_message(message: &str) -> String {
    let normalized = message.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "apple_intelligence_not_enabled" => humanize_apple_fm_unavailable_reason(
            AppleFmSystemLanguageModelUnavailableReason::AppleIntelligenceNotEnabled,
        ),
        "device_not_eligible" => humanize_apple_fm_unavailable_reason(
            AppleFmSystemLanguageModelUnavailableReason::DeviceNotEligible,
        ),
        "model_not_ready" | "downloading" | "preparing" => {
            humanize_apple_fm_unavailable_reason(
                AppleFmSystemLanguageModelUnavailableReason::ModelNotReady,
            )
        }
        "bridge_unreachable" => {
            "Apple Foundation Models bridge is not running. Build and launch `swift/foundation-bridge`.".to_string()
        }
        _ => format!("Apple Foundation Models unavailable: {}", message.trim()),
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProviderAutopilotPresenceState {
    pub last_published_mode: Option<String>,
    pub last_published_event_id: Option<String>,
    pub last_published_at_epoch_seconds: Option<u64>,
    pub last_expires_at_epoch_seconds: Option<u64>,
    pub pending_mode: Option<String>,
    pub pending_event_id: Option<String>,
    pub pending_queued_at: Option<Instant>,
    pub retry_after: Option<Instant>,
    pub next_heartbeat_at: Option<Instant>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

pub struct ProviderRuntimeState {
    pub mode: ProviderMode,
    pub mode_changed_at: Instant,
    pub connecting_until: Option<Instant>,
    pub online_since: Option<Instant>,
    pub inventory_session_started_at_ms: Option<i64>,
    pub defer_runtime_shutdown_until_idle: bool,
    pub last_heartbeat_at: Option<Instant>,
    pub heartbeat_interval: std::time::Duration,
    pub queue_depth: u32,
    pub last_completed_job_at: Option<Instant>,
    pub last_result: Option<String>,
    pub degraded_reason_code: Option<String>,
    pub last_error_detail: Option<String>,
    pub last_authoritative_status: Option<String>,
    pub last_authoritative_event_id: Option<String>,
    pub last_authoritative_error_class: Option<EarnFailureClass>,
    pub ingress_policy_filters: ProviderIngressPolicyFilterSummaryState,
    pub inventory_controls: ProviderInventoryControls,
    pub inventory_rows: Vec<ProviderInventoryRow>,
    pub inventory_last_action: Option<String>,
    pub inventory_last_error: Option<String>,
    pub gpt_oss: ProviderGptOssRuntimeState,
    pub apple_fm: ProviderAppleFmRuntimeState,
    pub autopilot_presence: ProviderAutopilotPresenceState,
    pub sandbox: ProviderSandboxAvailability,
    sandbox_detection: ProviderSandboxDetectionConfig,
    sandbox_last_scanned_at: Option<Instant>,
    sandbox_refresh_interval: Duration,
}

impl Default for ProviderRuntimeState {
    fn default() -> Self {
        let now = Instant::now();
        let sandbox_detection = ProviderSandboxDetectionConfig::default();
        let sandbox = detect_sandbox_supply(&sandbox_detection);
        Self {
            mode: ProviderMode::Offline,
            mode_changed_at: now,
            connecting_until: None,
            online_since: None,
            inventory_session_started_at_ms: None,
            defer_runtime_shutdown_until_idle: false,
            last_heartbeat_at: None,
            heartbeat_interval: std::time::Duration::from_secs(1),
            queue_depth: 0,
            last_completed_job_at: None,
            last_result: None,
            degraded_reason_code: None,
            last_error_detail: None,
            last_authoritative_status: None,
            last_authoritative_event_id: None,
            last_authoritative_error_class: None,
            ingress_policy_filters: ProviderIngressPolicyFilterSummaryState::default(),
            inventory_controls: ProviderInventoryControls::default(),
            inventory_rows: Vec::new(),
            inventory_last_action: Some(
                "Launch compute inventory not materialized yet".to_string(),
            ),
            inventory_last_error: None,
            gpt_oss: ProviderGptOssRuntimeState::default(),
            apple_fm: ProviderAppleFmRuntimeState::default(),
            autopilot_presence: ProviderAutopilotPresenceState::default(),
            sandbox,
            sandbox_detection,
            sandbox_last_scanned_at: Some(now),
            sandbox_refresh_interval: Duration::from_secs(30),
        }
    }
}

impl ProviderRuntimeState {
    pub const fn execution_lane_label(&self) -> &'static str {
        "compute"
    }

    pub fn availability(&self) -> ProviderAvailability {
        ProviderAvailability {
            gpt_oss: self.gpt_oss.substrate_health(),
            apple_foundation_models: self.apple_fm.substrate_health(),
            apple_adapter_hosting: self.apple_fm.substrate_adapter_hosting(),
            adapter_training_contributor: self.apple_fm.substrate_training_contributor(),
            sandbox: self.sandbox.clone(),
        }
    }

    pub fn derived_inventory_products(&self) -> Vec<ProviderAdvertisedProduct> {
        derive_provider_products(&self.availability(), &self.inventory_controls)
    }

    pub const fn backend_label(backend: LocalInferenceBackend) -> &'static str {
        backend.label()
    }

    pub fn active_inference_backend(&self) -> Option<LocalInferenceBackend> {
        self.availability().active_inference_backend()
    }

    pub fn execution_backend_label(&self) -> &'static str {
        self.availability().execution_backend_label()
    }

    pub const fn control_authority_label(&self, backend_authoritative: bool) -> &'static str {
        if backend_authoritative {
            "backend-authoritative"
        } else {
            "local only"
        }
    }

    pub const fn projection_authority_label(&self) -> &'static str {
        "projected / non-authoritative"
    }

    pub const fn settlement_truth_label(&self) -> &'static str {
        "wallet-authoritative"
    }

    pub fn uptime_seconds(&self, now: Instant) -> u64 {
        self.online_since
            .and_then(|started| now.checked_duration_since(started))
            .map_or(0, |duration| duration.as_secs())
    }

    pub fn heartbeat_age_seconds(&self, now: Instant) -> Option<u64> {
        self.last_heartbeat_at
            .and_then(|last| now.checked_duration_since(last))
            .map(|duration| duration.as_secs())
    }

    pub fn product_enabled(&self, product_id: &str) -> bool {
        self.inventory_controls.is_product_advertised(product_id)
    }

    pub fn toggle_inventory_target(
        &mut self,
        target: ProviderInventoryProductToggleTarget,
    ) -> bool {
        self.inventory_controls.toggle(target)
    }

    pub fn refresh_sandbox_supply_if_due(&mut self) -> bool {
        let should_refresh = self
            .sandbox_last_scanned_at
            .is_none_or(|last| last.elapsed() >= self.sandbox_refresh_interval);
        if !should_refresh {
            return false;
        }
        let next = detect_sandbox_supply(&self.sandbox_detection);
        self.sandbox_last_scanned_at = Some(Instant::now());
        if self.sandbox == next {
            return false;
        }
        self.sandbox = next;
        true
    }

    #[cfg(test)]
    pub fn sandbox_profiles(&self) -> &[openagents_provider_substrate::ProviderSandboxProfile] {
        self.sandbox.profiles.as_slice()
    }

    #[cfg(test)]
    pub fn sandbox_runtimes(
        &self,
    ) -> &[openagents_provider_substrate::ProviderSandboxRuntimeHealth] {
        self.sandbox.runtimes.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        APPLE_ADAPTER_REFERENCE_ADAPTER_FAMILY, APPLE_ADAPTER_REFERENCE_ADAPTER_FORMAT,
        APPLE_ADAPTER_REFERENCE_VALIDATOR_POLICY_REF, LocalInferenceBackend,
        ProviderAdapterTrainingExecutionBackend, ProviderAdapterTrainingSettlementTrigger,
        ProviderIngressPolicyFilterKind, ProviderIngressPolicyFilterSummaryState,
        ProviderInventoryControls, ProviderInventoryProductToggleTarget, ProviderRuntimeState,
    };
    use psionic_apple_fm::AppleFmSystemLanguageModelUnavailableReason;

    #[test]
    fn provider_runtime_truth_labels_distinguish_control_and_projection() {
        let runtime = ProviderRuntimeState::default();

        assert_eq!(runtime.execution_lane_label(), "compute");
        assert_eq!(
            runtime.execution_backend_label(),
            "no active inference backend"
        );
        assert_eq!(runtime.control_authority_label(false), "local only");
        assert_eq!(
            runtime.control_authority_label(true),
            "backend-authoritative"
        );
        assert_eq!(
            runtime.projection_authority_label(),
            "projected / non-authoritative"
        );
        assert_eq!(runtime.settlement_truth_label(), "wallet-authoritative");
    }

    #[test]
    fn ingress_policy_filter_summary_counts_expected_public_relay_noise() {
        let mut summary = ProviderIngressPolicyFilterSummaryState::default();

        assert_eq!(
            summary.record_reason(
                "unsupported request kind 5001; provider currently serves only kind 5050 text generation",
            ),
            Some(ProviderIngressPolicyFilterKind::UnsupportedKind)
        );
        assert_eq!(
            summary.record_reason(
                "request target policy mismatch (targets=[npub1other], local=[npub1local])",
            ),
            Some(ProviderIngressPolicyFilterKind::TargetMismatch)
        );
        assert_eq!(
            summary.record_reason("decrypt failed before payload parse"),
            None
        );
        assert_eq!(
            summary.provider_status_line().as_deref(),
            Some(
                "ingress policy filtered expected relay traffic (unsupported_kind=1, target_mismatch=1)",
            )
        );
        assert_eq!(
            summary.inbox_status_line().as_deref(),
            Some(
                "Policy-filtered live NIP-90 traffic is visible in Activity diagnostics (unsupported_kind=1, target_mismatch=1)",
            )
        );

        summary.clear();
        assert!(summary.is_empty());
        assert_eq!(summary.provider_status_line(), None);
        assert_eq!(summary.inbox_status_line(), None);
    }

    #[test]
    fn apple_backend_wins_when_ready() {
        let mut runtime = ProviderRuntimeState::default();
        runtime.gpt_oss.reachable = true;
        runtime.gpt_oss.ready_model = Some("llama3.2:latest".to_string());
        runtime.apple_fm.reachable = true;
        runtime.apple_fm.model_available = true;
        runtime.apple_fm.ready_model = Some("apple-foundation-model".to_string());

        assert_eq!(
            runtime.active_inference_backend(),
            Some(LocalInferenceBackend::AppleFoundationModels)
        );
    }

    #[test]
    fn apple_runtime_projects_adapter_inventory_into_substrate_availability() {
        let mut runtime = ProviderRuntimeState::default();
        runtime.apple_fm.reachable = true;
        runtime.apple_fm.model_available = true;
        runtime.apple_fm.ready_model = Some("apple-foundation-model".to_string());
        runtime.apple_fm.adapter_inventory_supported = true;
        runtime.apple_fm.adapter_attach_supported = true;
        runtime
            .apple_fm
            .loaded_adapters
            .push(psionic_apple_fm::AppleFmAdapterInventoryEntry {
                adapter: psionic_apple_fm::AppleFmAdapterSelection {
                    adapter_id: "helpdesk".to_string(),
                    package_digest: Some("sha256:helpdesk".to_string()),
                },
                base_model_signature: Some("apple.fm.base".to_string()),
                package_format_version: Some("fmadapter.v1".to_string()),
                draft_model_present: true,
                compatibility: psionic_apple_fm::AppleFmAdapterCompatibility {
                    compatible: true,
                    reason_code: None,
                    message: Some("compatible with the current Apple FM runtime".to_string()),
                },
                attached_session_ids: vec!["sess-1".to_string(), "sess-2".to_string()],
            });

        let availability = runtime.availability();

        assert!(availability.apple_adapter_hosting.inventory_supported);
        assert!(availability.apple_adapter_hosting.attach_supported);
        assert_eq!(availability.apple_adapter_hosting.loaded_adapter_count(), 1);
        assert_eq!(
            availability
                .apple_adapter_hosting
                .compatible_adapter_count(),
            1
        );
        assert_eq!(
            availability.apple_adapter_hosting.adapters[0]
                .package_digest
                .as_deref(),
            Some("sha256:helpdesk")
        );
        assert_eq!(
            availability.apple_adapter_hosting.adapters[0].attached_session_count,
            2
        );
    }

    #[test]
    fn apple_runtime_projects_training_contributor_into_substrate_availability() {
        let mut runtime = ProviderRuntimeState::default();
        runtime.apple_fm.reachable = true;
        runtime.apple_fm.model_available = true;
        runtime.apple_fm.ready_model = Some("apple-foundation-model".to_string());

        let availability = runtime.availability();

        assert!(
            availability
                .adapter_training_contributor
                .contributor_supported
        );
        assert!(
            availability
                .adapter_training_contributor
                .coordinator_match_supported
        );
        assert!(
            availability
                .adapter_training_contributor
                .authority_receipt_supported
        );
        assert_eq!(
            availability.adapter_training_contributor.execution_backends,
            vec![ProviderAdapterTrainingExecutionBackend::AppleFoundationModels]
        );
        assert_eq!(
            availability.adapter_training_contributor.adapter_families,
            vec![APPLE_ADAPTER_REFERENCE_ADAPTER_FAMILY.to_string()]
        );
        assert_eq!(
            availability.adapter_training_contributor.adapter_formats,
            vec![APPLE_ADAPTER_REFERENCE_ADAPTER_FORMAT.to_string()]
        );
        assert_eq!(
            availability
                .adapter_training_contributor
                .validator_policy_refs,
            vec![APPLE_ADAPTER_REFERENCE_VALIDATOR_POLICY_REF.to_string()]
        );
        assert_eq!(
            availability.adapter_training_contributor.settlement_trigger,
            Some(ProviderAdapterTrainingSettlementTrigger::AcceptedContribution)
        );
    }

    #[test]
    fn inventory_controls_gate_launch_products_by_product_id() {
        let mut controls = ProviderInventoryControls::default();
        assert!(controls.is_product_advertised("gpt_oss.text_generation"));
        assert!(!controls.is_product_advertised("gpt_oss.embeddings"));
        assert!(controls.is_product_advertised("apple_foundation_models.text_generation"));
        assert!(!controls.is_product_advertised("sandbox.python.exec"));

        let enabled = controls.toggle(ProviderInventoryProductToggleTarget::GptOssEmbeddings);
        assert!(!enabled);
        assert!(!controls.is_product_advertised("gpt_oss.embeddings"));
        assert!(controls.is_product_advertised("gpt_oss.text_generation"));
    }

    #[test]
    fn runtime_detects_sandbox_runtimes_even_without_declared_profiles() {
        let runtime = ProviderRuntimeState::default();

        assert!(!runtime.sandbox_runtimes().is_empty());
        assert!(runtime.sandbox_profiles().is_empty());
    }

    #[test]
    fn apple_fm_availability_error_ignores_positive_health_message() {
        let runtime = super::ProviderAppleFmRuntimeState {
            reachable: true,
            model_available: true,
            ready_model: None,
            availability_message: Some("Foundation Models is available".to_string()),
            ..super::ProviderAppleFmRuntimeState::default()
        };

        assert_eq!(runtime.availability_error_message(), None);
        assert_eq!(
            runtime.readiness_block_reason().as_deref(),
            Some("Apple Foundation Models bridge reachable; waiting for model inventory.")
        );
    }

    #[test]
    fn apple_fm_availability_error_uses_unavailable_reason() {
        let runtime = super::ProviderAppleFmRuntimeState {
            reachable: true,
            model_available: false,
            unavailable_reason: Some(
                AppleFmSystemLanguageModelUnavailableReason::AppleIntelligenceNotEnabled,
            ),
            ..super::ProviderAppleFmRuntimeState::default()
        };

        assert_eq!(
            runtime.availability_error_message().as_deref(),
            Some(
                "Apple Intelligence is disabled. Enable it in System Settings > Apple Intelligence."
            )
        );
    }

    #[test]
    fn apple_fm_availability_error_humanizes_model_not_ready() {
        let runtime = super::ProviderAppleFmRuntimeState {
            reachable: true,
            model_available: false,
            unavailable_reason: Some(AppleFmSystemLanguageModelUnavailableReason::ModelNotReady),
            ..super::ProviderAppleFmRuntimeState::default()
        };

        assert_eq!(
            runtime.availability_error_message().as_deref(),
            Some("Apple Foundation Models is still downloading or preparing the system model.")
        );
    }

    #[test]
    fn apple_fm_readiness_block_reason_explains_missing_bridge() {
        let runtime = super::ProviderAppleFmRuntimeState::default();
        assert_eq!(
            runtime.readiness_block_reason().as_deref(),
            Some(
                "Apple Foundation Models bridge is not running. Build and launch `swift/foundation-bridge`."
            )
        );
    }

    #[test]
    fn apple_fm_availability_error_prefers_explicit_last_error() {
        let runtime = super::ProviderAppleFmRuntimeState {
            reachable: true,
            model_available: true,
            ready_model: Some("apple-foundation-model".to_string()),
            availability_message: Some("Foundation Models is available".to_string()),
            last_error: Some("bridge request timed out".to_string()),
            ..super::ProviderAppleFmRuntimeState::default()
        };

        assert_eq!(
            runtime.availability_error_message().as_deref(),
            Some("bridge request timed out")
        );
    }

    #[test]
    fn apple_fm_capability_state_is_not_authoritative_before_first_health_result() {
        let runtime = super::ProviderAppleFmRuntimeState::default();
        assert!(!runtime.has_authoritative_capability_state());
    }

    #[test]
    fn apple_fm_capability_state_is_authoritative_once_ready() {
        let runtime = super::ProviderAppleFmRuntimeState {
            reachable: true,
            model_available: true,
            ready_model: Some("apple-foundation-model".to_string()),
            bridge_status: Some("running".to_string()),
            ..super::ProviderAppleFmRuntimeState::default()
        };
        assert!(runtime.has_authoritative_capability_state());
    }

    #[test]
    fn apple_fm_capability_state_is_authoritative_once_unavailable_reason_is_known() {
        let runtime = super::ProviderAppleFmRuntimeState {
            reachable: true,
            model_available: false,
            unavailable_reason: Some(
                AppleFmSystemLanguageModelUnavailableReason::AppleIntelligenceNotEnabled,
            ),
            bridge_status: Some("running".to_string()),
            ..super::ProviderAppleFmRuntimeState::default()
        };
        assert!(runtime.has_authoritative_capability_state());
    }
}
