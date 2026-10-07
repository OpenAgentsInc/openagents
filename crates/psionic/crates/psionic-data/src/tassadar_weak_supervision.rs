use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_WEAK_SUPERVISION_ABI_VERSION: &str =
    "psionic.tassadar.weak_supervision.v1";
pub const TASSADAR_WEAK_SUPERVISION_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/weak_supervision_executor";
pub const TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_weak_supervision_executor_v1/weak_supervision_evidence_bundle.json";
pub const TASSADAR_WEAK_SUPERVISION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_weak_supervision_executor_report.json";
pub const TASSADAR_WEAK_SUPERVISION_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_weak_supervision_executor_summary.json";

/// Supervision regime admitted by the weak-supervision executor family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWeakSupervisionRegime {
    /// Full token-by-token trace supervision.
    FullTrace,
    /// Mixed IO, invariant, partial-state, and subroutine-label supervision.
    MixedWeak,
    /// IO-only targets with refusal calibration.
    IoOnly,
}

impl TassadarWeakSupervisionRegime {
    /// Returns the stable regime label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullTrace => "full_trace",
            Self::MixedWeak => "mixed_weak",
            Self::IoOnly => "io_only",
        }
    }
}

/// Signal component surfaced by the weak-supervision contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWeakSupervisionSignal {
    IoTargets,
    Invariants,
    PartialState,
    SubroutineLabels,
    FullTraceTokens,
}

/// Module-scale workload family tracked by the weak-supervision lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarWeakSupervisionWorkloadFamily {
    ModuleTraceV2,
    HungarianModule,
    VerifierSearchKernel,
    ModuleStateControl,
}

impl TassadarWeakSupervisionWorkloadFamily {
    /// Returns the stable workload label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ModuleTraceV2 => "module_trace_v2",
            Self::HungarianModule => "hungarian_module",
            Self::VerifierSearchKernel => "verifier_search_kernel",
            Self::ModuleStateControl => "module_state_control",
        }
    }
}

/// One workload row in the public weak-supervision contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionWorkloadRow {
    /// Compared workload family.
    pub workload_family: TassadarWeakSupervisionWorkloadFamily,
    /// Supervision regimes compared on the workload.
    pub compared_regimes: Vec<TassadarWeakSupervisionRegime>,
    /// Signal components available in the mixed regime.
    pub mixed_signal_components: Vec<TassadarWeakSupervisionSignal>,
    /// Existing authority refs anchoring the workload.
    pub authority_refs: Vec<String>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
}

/// Public contract for the weak-supervision learned executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable contract ref.
    pub contract_ref: String,
    /// Stable version label.
    pub version: String,
    /// Supported supervision regimes.
    pub regimes: Vec<TassadarWeakSupervisionRegime>,
    /// Supported signal components.
    pub signal_components: Vec<TassadarWeakSupervisionSignal>,
    /// Workload rows compared by the lane.
    pub workload_rows: Vec<TassadarWeakSupervisionWorkloadRow>,
    /// Evaluation axes surfaced by the committed artifacts.
    pub evaluation_axes: Vec<String>,
    /// Train evidence bundle ref.
    pub evidence_bundle_ref: String,
    /// Eval report ref.
    pub report_ref: String,
    /// Research summary ref.
    pub summary_report_ref: String,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

impl TassadarWeakSupervisionContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_WEAK_SUPERVISION_ABI_VERSION),
            contract_ref: String::from(TASSADAR_WEAK_SUPERVISION_CONTRACT_REF),
            version: String::from("2026.03.18"),
            regimes: vec![
                TassadarWeakSupervisionRegime::FullTrace,
                TassadarWeakSupervisionRegime::MixedWeak,
                TassadarWeakSupervisionRegime::IoOnly,
            ],
            signal_components: vec![
                TassadarWeakSupervisionSignal::IoTargets,
                TassadarWeakSupervisionSignal::Invariants,
                TassadarWeakSupervisionSignal::PartialState,
                TassadarWeakSupervisionSignal::SubroutineLabels,
                TassadarWeakSupervisionSignal::FullTraceTokens,
            ],
            workload_rows: workload_rows(),
            evaluation_axes: vec![
                String::from("later_window_exactness_bps"),
                String::from("refusal_calibration_bps"),
                String::from("final_output_exactness_bps"),
                String::from("under_supervised_failure_count"),
            ],
            evidence_bundle_ref: String::from(TASSADAR_WEAK_SUPERVISION_EVIDENCE_BUNDLE_REF),
            report_ref: String::from(TASSADAR_WEAK_SUPERVISION_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_WEAK_SUPERVISION_SUMMARY_REPORT_REF),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("weak-supervision contract should validate");
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_weak_supervision_contract|", &contract);
        contract
    }

    /// Validates the public contract.
    pub fn validate(&self) -> Result<(), TassadarWeakSupervisionContractError> {
        if self.abi_version != TASSADAR_WEAK_SUPERVISION_ABI_VERSION {
            return Err(TassadarWeakSupervisionContractError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarWeakSupervisionContractError::MissingContractRef);
        }
        if self.regimes.is_empty() {
            return Err(TassadarWeakSupervisionContractError::MissingRegimes);
        }
        if self.signal_components.is_empty() {
            return Err(TassadarWeakSupervisionContractError::MissingSignals);
        }
        if self.workload_rows.is_empty() {
            return Err(TassadarWeakSupervisionContractError::MissingWorkloads);
        }
        if self.evidence_bundle_ref.trim().is_empty()
            || self.report_ref.trim().is_empty()
            || self.summary_report_ref.trim().is_empty()
        {
            return Err(TassadarWeakSupervisionContractError::MissingReportRefs);
        }
        Ok(())
    }
}

/// Case-level evidence row in the committed train artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionEvidenceCase {
    /// Stable case identifier.
    pub case_id: String,
    /// Compared workload family.
    pub workload_family: TassadarWeakSupervisionWorkloadFamily,
    /// Compared supervision regime.
    pub supervision_regime: TassadarWeakSupervisionRegime,
    /// Later-window exactness in basis points.
    pub later_window_exactness_bps: u32,
    /// Final-output exactness in basis points.
    pub final_output_exactness_bps: u32,
    /// Refusal calibration in basis points.
    pub refusal_calibration_bps: u32,
    /// Count of explicit under-supervised failure events on the seeded run.
    pub under_supervised_failure_count: u32,
    /// Dominant under-supervised failure mode.
    pub dominant_failure_mode: String,
    /// Plain-language note.
    pub note: String,
}

/// Train-side evidence bundle for the weak-supervision lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWeakSupervisionEvidenceBundle {
    /// Public contract for the lane.
    pub contract: TassadarWeakSupervisionContract,
    /// Case-level evidence rows.
    pub case_reports: Vec<TassadarWeakSupervisionEvidenceCase>,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub report_digest: String,
}

fn workload_rows() -> Vec<TassadarWeakSupervisionWorkloadRow> {
    let compared_regimes = vec![
        TassadarWeakSupervisionRegime::FullTrace,
        TassadarWeakSupervisionRegime::MixedWeak,
        TassadarWeakSupervisionRegime::IoOnly,
    ];
    let mixed_signals = vec![
        TassadarWeakSupervisionSignal::IoTargets,
        TassadarWeakSupervisionSignal::Invariants,
        TassadarWeakSupervisionSignal::PartialState,
        TassadarWeakSupervisionSignal::SubroutineLabels,
    ];
    vec![
        TassadarWeakSupervisionWorkloadRow {
            workload_family: TassadarWeakSupervisionWorkloadFamily::ModuleTraceV2,
            compared_regimes: compared_regimes.clone(),
            mixed_signal_components: mixed_signals.clone(),
            authority_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_module_trace_abi_v2_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_state_design_study_report.json"),
            ],
            claim_boundary: String::from(
                "module-trace-v2 remains bounded to the current machine-legible frame and call-trace authorities; weak supervision here does not imply arbitrary module execution closure",
            ),
        },
        TassadarWeakSupervisionWorkloadRow {
            workload_family: TassadarWeakSupervisionWorkloadFamily::HungarianModule,
            compared_regimes: compared_regimes.clone(),
            mixed_signal_components: mixed_signals.clone(),
            authority_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_compiled_distillation_report.json"),
            ],
            claim_boundary: String::from(
                "Hungarian module supervision remains bounded to the current module-scale family and does not widen compiled exactness or served claims",
            ),
        },
        TassadarWeakSupervisionWorkloadRow {
            workload_family: TassadarWeakSupervisionWorkloadFamily::VerifierSearchKernel,
            compared_regimes: compared_regimes.clone(),
            mixed_signal_components: mixed_signals.clone(),
            authority_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_error_regime_catalog.json"),
            ],
            claim_boundary: String::from(
                "verifier-search supervision remains bounded to the current search-kernel witnesses and explicit contradiction semantics",
            ),
        },
        TassadarWeakSupervisionWorkloadRow {
            workload_family: TassadarWeakSupervisionWorkloadFamily::ModuleStateControl,
            compared_regimes,
            mixed_signal_components: mixed_signals,
            authority_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_module_state_architecture_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_state_design_study_report.json"),
            ],
            claim_boundary: String::from(
                "module-state control remains a bounded later-window family with explicit refusal when weaker supervision cannot preserve the declared carried-state semantics",
            ),
        },
    ]
}

/// Contract validation failure.
#[derive(Debug, Error)]
pub enum TassadarWeakSupervisionContractError {
    #[error("unsupported weak-supervision ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("weak-supervision contract is missing `contract_ref`")]
    MissingContractRef,
    #[error("weak-supervision contract must declare regimes")]
    MissingRegimes,
    #[error("weak-supervision contract must declare signal components")]
    MissingSignals,
    #[error("weak-supervision contract must declare workloads")]
    MissingWorkloads,
    #[error("weak-supervision contract must declare report refs")]
    MissingReportRefs,
}

/// Returns the canonical weak-supervision contract.
#[must_use]
pub fn tassadar_weak_supervision_contract() -> TassadarWeakSupervisionContract {
    TassadarWeakSupervisionContract::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        tassadar_weak_supervision_contract, TassadarWeakSupervisionRegime,
        TassadarWeakSupervisionSignal, TassadarWeakSupervisionWorkloadFamily,
        TASSADAR_WEAK_SUPERVISION_ABI_VERSION,
    };

    #[test]
    fn weak_supervision_contract_is_machine_legible() {
        let contract = tassadar_weak_supervision_contract();

        assert_eq!(contract.abi_version, TASSADAR_WEAK_SUPERVISION_ABI_VERSION);
        assert!(contract
            .regimes
            .contains(&TassadarWeakSupervisionRegime::MixedWeak));
        assert!(contract
            .signal_components
            .contains(&TassadarWeakSupervisionSignal::PartialState));
        assert!(contract
            .workload_rows
            .iter()
            .any(|row| row.workload_family == TassadarWeakSupervisionWorkloadFamily::ModuleStateControl));
        assert!(!contract.contract_digest.is_empty());
    }
}
