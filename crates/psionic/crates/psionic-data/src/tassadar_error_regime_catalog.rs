use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TassadarErrorRegimeClass, TassadarErrorRegimeReceipt, TassadarErrorRegimeRecoverySurface,
    TassadarErrorRegimeWorkloadFamily,
};

pub const TASSADAR_ERROR_REGIME_CATALOG_ABI_VERSION: &str =
    "psionic.tassadar.error_regime_catalog.v1";
pub const TASSADAR_ERROR_REGIME_CATALOG_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/error_regime_catalog";
pub const TASSADAR_ERROR_REGIME_SWEEP_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/tassadar_error_regime_catalog_v1";
pub const TASSADAR_ERROR_REGIME_SWEEP_REPORT_FILE: &str = "error_regime_sweep_report.json";
pub const TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF: &str =
    "fixtures/tassadar/runs/tassadar_error_regime_catalog_v1/error_regime_sweep_report.json";
pub const TASSADAR_ERROR_REGIME_CATALOG_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_error_regime_catalog.json";
pub const TASSADAR_ERROR_REGIME_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_error_regime_summary.json";

/// One seeded workload row in the public error-regime contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeCaseContract {
    /// Stable case identifier.
    pub case_id: String,
    /// Compared workload family.
    pub workload_family: TassadarErrorRegimeWorkloadFamily,
    /// Step where the seeded error is injected.
    pub injected_error_step: u32,
    /// Declared checkpoint spacing in steps.
    pub checkpoint_spacing_steps: u32,
    /// Plain-language workload note.
    pub note: String,
}

impl TassadarErrorRegimeCaseContract {
    fn validate(&self) -> Result<(), TassadarErrorRegimeCatalogContractError> {
        if self.case_id.trim().is_empty() {
            return Err(TassadarErrorRegimeCatalogContractError::MissingCaseId);
        }
        if self.injected_error_step == 0 {
            return Err(
                TassadarErrorRegimeCatalogContractError::InvalidInjectedErrorStep {
                    case_id: self.case_id.clone(),
                },
            );
        }
        if self.checkpoint_spacing_steps == 0 {
            return Err(
                TassadarErrorRegimeCatalogContractError::InvalidCheckpointSpacing {
                    case_id: self.case_id.clone(),
                },
            );
        }
        if self.note.trim().is_empty() {
            return Err(TassadarErrorRegimeCatalogContractError::MissingCaseNote {
                case_id: self.case_id.clone(),
            });
        }
        Ok(())
    }
}

/// Public contract for the error-regime catalog lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeCatalogContract {
    /// Stable ABI version.
    pub abi_version: String,
    /// Stable contract reference.
    pub contract_ref: String,
    /// Immutable version label.
    pub version: String,
    /// Supported workload families.
    pub workload_families: Vec<TassadarErrorRegimeWorkloadFamily>,
    /// Supported recovery surfaces.
    pub recovery_surfaces: Vec<TassadarErrorRegimeRecoverySurface>,
    /// Supported regime classes.
    pub regime_classes: Vec<TassadarErrorRegimeClass>,
    /// Evaluation axes surfaced by the committed artifacts.
    pub evaluation_axes: Vec<String>,
    /// Seeded error-injection cases.
    pub cases: Vec<TassadarErrorRegimeCaseContract>,
    /// Train-side committed run artifact ref.
    pub sweep_report_ref: String,
    /// Eval-side committed catalog ref.
    pub catalog_report_ref: String,
    /// Research-side committed summary ref.
    pub summary_report_ref: String,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

impl TassadarErrorRegimeCatalogContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_ERROR_REGIME_CATALOG_ABI_VERSION),
            contract_ref: String::from(TASSADAR_ERROR_REGIME_CATALOG_CONTRACT_REF),
            version: String::from("2026.03.18"),
            workload_families: vec![
                TassadarErrorRegimeWorkloadFamily::SudokuBacktracking,
                TassadarErrorRegimeWorkloadFamily::SearchKernel,
                TassadarErrorRegimeWorkloadFamily::LongHorizonControl,
                TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop,
            ],
            recovery_surfaces: vec![
                TassadarErrorRegimeRecoverySurface::Uncorrected,
                TassadarErrorRegimeRecoverySurface::CheckpointOnly,
                TassadarErrorRegimeRecoverySurface::VerifierOnly,
                TassadarErrorRegimeRecoverySurface::CheckpointAndVerifier,
            ],
            regime_classes: vec![
                TassadarErrorRegimeClass::SelfHealing,
                TassadarErrorRegimeClass::SlowDrift,
                TassadarErrorRegimeClass::CatastrophicDivergence,
            ],
            evaluation_axes: vec![
                String::from("recovered_exactness_bps"),
                String::from("divergence_step"),
                String::from("checkpoint_reset_count"),
                String::from("verifier_intervention_count"),
                String::from("correction_event_count"),
            ],
            cases: vec![
                TassadarErrorRegimeCaseContract {
                    case_id: String::from("sudoku_backtracking_error_injection"),
                    workload_family: TassadarErrorRegimeWorkloadFamily::SudokuBacktracking,
                    injected_error_step: 7,
                    checkpoint_spacing_steps: 4,
                    note: String::from(
                        "bounded Sudoku backtracking with branch-level contradiction handling",
                    ),
                },
                TassadarErrorRegimeCaseContract {
                    case_id: String::from("search_kernel_error_injection"),
                    workload_family: TassadarErrorRegimeWorkloadFamily::SearchKernel,
                    injected_error_step: 5,
                    checkpoint_spacing_steps: 3,
                    note: String::from(
                        "small verifier-guided branch search with a narrow frontier budget",
                    ),
                },
                TassadarErrorRegimeCaseContract {
                    case_id: String::from("long_horizon_control_error_injection"),
                    workload_family: TassadarErrorRegimeWorkloadFamily::LongHorizonControl,
                    injected_error_step: 18,
                    checkpoint_spacing_steps: 6,
                    note: String::from(
                        "long-horizon carried-state control with later-window drift and partial correction",
                    ),
                },
                TassadarErrorRegimeCaseContract {
                    case_id: String::from("byte_memory_loop_error_injection"),
                    workload_family: TassadarErrorRegimeWorkloadFamily::ByteMemoryLoop,
                    injected_error_step: 11,
                    checkpoint_spacing_steps: 5,
                    note: String::from(
                        "byte-addressed mutation loop where bad writes remain replay-visible and rewindable",
                    ),
                },
            ],
            sweep_report_ref: String::from(TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF),
            catalog_report_ref: String::from(TASSADAR_ERROR_REGIME_CATALOG_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_ERROR_REGIME_SUMMARY_REPORT_REF),
            claim_boundary: String::from(
                "this contract defines one bounded error-regime catalog over declared workloads, correction surfaces, and regime classes. It keeps checkpoint, verifier, and correction claims explicit instead of treating recovery machinery as general exactness proof",
            ),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("error-regime catalog contract should validate");
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_error_regime_catalog_contract|",
            &contract,
        );
        contract
    }

    /// Validates the public contract.
    pub fn validate(&self) -> Result<(), TassadarErrorRegimeCatalogContractError> {
        if self.abi_version != TASSADAR_ERROR_REGIME_CATALOG_ABI_VERSION {
            return Err(
                TassadarErrorRegimeCatalogContractError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarErrorRegimeCatalogContractError::MissingContractRef);
        }
        if self.workload_families.is_empty() {
            return Err(TassadarErrorRegimeCatalogContractError::MissingWorkloadFamilies);
        }
        if self.recovery_surfaces.is_empty() {
            return Err(TassadarErrorRegimeCatalogContractError::MissingRecoverySurfaces);
        }
        if self.regime_classes.is_empty() {
            return Err(TassadarErrorRegimeCatalogContractError::MissingRegimeClasses);
        }
        if self.cases.is_empty() {
            return Err(TassadarErrorRegimeCatalogContractError::MissingCases);
        }
        if self.sweep_report_ref.trim().is_empty()
            || self.catalog_report_ref.trim().is_empty()
            || self.summary_report_ref.trim().is_empty()
        {
            return Err(TassadarErrorRegimeCatalogContractError::MissingReportRefs);
        }
        for case in &self.cases {
            case.validate()?;
        }
        Ok(())
    }
}

/// Per-workload summary emitted by the train-side sweep artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeWorkloadSweepSummary {
    /// Compared workload family.
    pub workload_family: TassadarErrorRegimeWorkloadFamily,
    /// Recovery surface with the highest exactness on the seeded sweep.
    pub best_recovery_surface: TassadarErrorRegimeRecoverySurface,
    /// Exactness delta for verifier-only over uncorrected.
    pub verifier_only_exactness_delta_bps: i32,
    /// Exactness delta for checkpoint-only over uncorrected.
    pub checkpoint_only_exactness_delta_bps: i32,
    /// Exactness delta for combined recovery over uncorrected.
    pub combined_exactness_delta_bps: i32,
    /// Number of self-healing surfaces for the workload.
    pub self_healing_surface_count: u32,
    /// Number of catastrophic surfaces for the workload.
    pub catastrophic_surface_count: u32,
    /// Plain-language note.
    pub note: String,
}

/// Train-side committed sweep artifact for the error-regime study.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarErrorRegimeSweepReport {
    /// Stable version label.
    pub version: String,
    /// Public catalog contract.
    pub catalog_contract: TassadarErrorRegimeCatalogContract,
    /// Runtime receipts for the injected-error sweep.
    pub runtime_receipts: Vec<TassadarErrorRegimeReceipt>,
    /// Workload-level sweep summaries.
    pub workload_summaries: Vec<TassadarErrorRegimeWorkloadSweepSummary>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Plain-language report summary.
    pub summary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

/// Validation failure for the public error-regime contract.
#[derive(Debug, Error)]
pub enum TassadarErrorRegimeCatalogContractError {
    #[error("unsupported ABI version `{abi_version}` for error-regime catalog contract")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("error-regime catalog contract is missing `contract_ref`")]
    MissingContractRef,
    #[error("error-regime catalog contract is missing workload families")]
    MissingWorkloadFamilies,
    #[error("error-regime catalog contract is missing recovery surfaces")]
    MissingRecoverySurfaces,
    #[error("error-regime catalog contract is missing regime classes")]
    MissingRegimeClasses,
    #[error("error-regime catalog contract is missing cases")]
    MissingCases,
    #[error("error-regime catalog contract is missing report refs")]
    MissingReportRefs,
    #[error("error-regime catalog case is missing `case_id`")]
    MissingCaseId,
    #[error("error-regime catalog case `{case_id}` has invalid `injected_error_step=0`")]
    InvalidInjectedErrorStep { case_id: String },
    #[error("error-regime catalog case `{case_id}` has invalid `checkpoint_spacing_steps=0`")]
    InvalidCheckpointSpacing { case_id: String },
    #[error("error-regime catalog case `{case_id}` is missing a note")]
    MissingCaseNote { case_id: String },
}

/// Returns the canonical public contract for the error-regime catalog.
#[must_use]
pub fn tassadar_error_regime_catalog_contract() -> TassadarErrorRegimeCatalogContract {
    TassadarErrorRegimeCatalogContract::new()
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
        TASSADAR_ERROR_REGIME_CATALOG_CONTRACT_REF, TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF,
        tassadar_error_regime_catalog_contract,
    };
    use psionic_runtime::TassadarErrorRegimeWorkloadFamily;

    #[test]
    fn error_regime_catalog_contract_is_machine_legible() {
        let contract = tassadar_error_regime_catalog_contract();

        assert_eq!(
            contract.contract_ref,
            TASSADAR_ERROR_REGIME_CATALOG_CONTRACT_REF
        );
        assert_eq!(contract.cases.len(), 4);
        assert!(
            contract
                .workload_families
                .contains(&TassadarErrorRegimeWorkloadFamily::LongHorizonControl)
        );
        assert_eq!(
            contract.sweep_report_ref,
            TASSADAR_ERROR_REGIME_SWEEP_REPORT_REF
        );
        assert!(!contract.contract_digest.is_empty());
    }
}
