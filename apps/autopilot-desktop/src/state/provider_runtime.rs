//! Runtime-facing provider state for the embedded OpenAgents Runtime.
//!
//! This module keeps app-owned execution snapshots and UX-facing state local to
//! `autopilot-desktop`, while reusing the narrow shared provider substrate for
//! backend identity, launch-product derivation, inventory controls, and
//! provider lifecycle semantics.

use std::time::{Duration, Instant};

use crate::local_inference_runtime::LocalInferenceExecutionMetrics;
pub use openagents_provider_substrate::{
    ProviderAdvertisedProduct, ProviderAvailability, ProviderBackendHealth, ProviderBackendKind,
    ProviderBlocker, ProviderComputeProduct, ProviderFailureClass, ProviderInventoryControls,
    ProviderInventoryRow, ProviderMode, ProviderSandboxAvailability,
    ProviderSandboxDetectionConfig, derive_provider_products, detect_sandbox_supply,
};
use psionic_apple_fm::{
    AppleFmAdapterInventoryEntry, AppleFmSystemLanguageModel, AppleFmSystemLanguageModelGuardrails,
    AppleFmSystemLanguageModelUnavailableReason, AppleFmSystemLanguageModelUseCase,
};

pub type LocalInferenceBackend = ProviderBackendKind;
pub type EarnFailureClass = ProviderFailureClass;
pub type ProviderInventoryProductToggleTarget = ProviderComputeProduct;

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
                        .or_else(|| {
                            self.unavailable_reason
                                .map(|reason| format!("Apple FM unavailable: {}", reason.label()))
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
            if self.reachable {
                Some(
                    "Apple Foundation Models bridge reachable; waiting for model inventory."
                        .to_string(),
                )
            } else {
                Some("Apple Foundation Models bridge is not running.".to_string())
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
}

fn is_positive_apple_fm_availability_message(message: &str) -> bool {
    message
        .trim()
        .eq_ignore_ascii_case("Foundation Models is available")
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
        LocalInferenceBackend, ProviderInventoryControls, ProviderInventoryProductToggleTarget,
        ProviderRuntimeState,
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
            Some("Apple FM unavailable: apple_intelligence_not_enabled")
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
