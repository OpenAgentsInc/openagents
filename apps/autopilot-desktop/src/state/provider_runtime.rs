//! Runtime-facing provider state for the embedded OpenAgents Runtime.
//!
//! This module models execution and operational health on the worker/client side.
//! It is not economic authority: settlement truth, verification verdicts, and
//! canonical receipts remain kernel- or wallet-authoritative elsewhere.

use std::time::Instant;

use crate::ollama_execution::OllamaExecutionMetrics;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderBlocker {
    IdentityMissing,
    WalletError,
    SkillTrustUnavailable,
    CreditLaneUnavailable,
    OllamaUnavailable,
    OllamaModelUnavailable,
}

impl ProviderBlocker {
    pub const fn code(self) -> &'static str {
        match self {
            Self::IdentityMissing => "IDENTITY_MISSING",
            Self::WalletError => "WALLET_ERROR",
            Self::SkillTrustUnavailable => "SKL_TRUST_UNAVAILABLE",
            Self::CreditLaneUnavailable => "AC_CREDIT_UNAVAILABLE",
            Self::OllamaUnavailable => "OLLAMA_UNAVAILABLE",
            Self::OllamaModelUnavailable => "OLLAMA_MODEL_UNAVAILABLE",
        }
    }

    pub const fn detail(self) -> &'static str {
        match self {
            Self::IdentityMissing => "Nostr identity is not ready",
            Self::WalletError => "Spark wallet reports an error",
            Self::SkillTrustUnavailable => "SKL trust gate is not trusted",
            Self::CreditLaneUnavailable => "AC credit lane is not available",
            Self::OllamaUnavailable => "Local Ollama backend is unavailable",
            Self::OllamaModelUnavailable => "No local Ollama serving model is ready",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EarnFailureClass {
    Relay,
    Execution,
    Payment,
    Reconciliation,
}

impl EarnFailureClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Relay => "relay",
            Self::Execution => "execution",
            Self::Payment => "payment",
            Self::Reconciliation => "reconciliation",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProviderOllamaRuntimeState {
    pub reachable: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub loaded_models: Vec<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_request_id: Option<String>,
    pub last_metrics: Option<OllamaExecutionMetrics>,
    pub refreshed_at: Option<Instant>,
}

impl ProviderOllamaRuntimeState {
    pub fn is_ready(&self) -> bool {
        self.reachable && self.ready_model.is_some()
    }
}

pub struct ProviderRuntimeState {
    pub mode: ProviderMode,
    pub mode_changed_at: Instant,
    pub connecting_until: Option<Instant>,
    pub online_since: Option<Instant>,
    pub inventory_session_started_at_ms: Option<i64>,
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
    pub ollama: ProviderOllamaRuntimeState,
}

impl Default for ProviderRuntimeState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            mode: ProviderMode::Offline,
            mode_changed_at: now,
            connecting_until: None,
            online_since: None,
            inventory_session_started_at_ms: None,
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
            ollama: ProviderOllamaRuntimeState::default(),
        }
    }
}

impl ProviderRuntimeState {
    pub const fn execution_lane_label(&self) -> &'static str {
        "compute"
    }

    pub const fn execution_backend_label(&self) -> &'static str {
        "local Ollama runtime"
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
}

#[cfg(test)]
mod tests {
    use super::ProviderRuntimeState;

    #[test]
    fn provider_runtime_truth_labels_distinguish_control_and_projection() {
        let runtime = ProviderRuntimeState::default();

        assert_eq!(runtime.execution_lane_label(), "compute");
        assert_eq!(runtime.execution_backend_label(), "local Ollama runtime");
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
}
