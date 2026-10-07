use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_COMPILED_DISTILLATION_ABI_VERSION: &str =
    "psionic.tassadar.compiled_distillation.v1";
pub const TASSADAR_COMPILED_DISTILLATION_TARGET_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_compiled_distillation_targets_v1/compiled_distillation_target_bundle.json";
pub const TASSADAR_COMPILED_DISTILLATION_TRAINING_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_compiled_distillation_v1/compiled_distillation_training_evidence_bundle.json";
pub const TASSADAR_COMPILED_DISTILLATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_compiled_distillation_report.json";
pub const TASSADAR_COMPILED_DISTILLATION_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_compiled_distillation_summary.json";

/// Supervision mode admitted by the compiled-to-learned distillation lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledDistillationMode {
    FullTrace,
    IoOnly,
    PartialState,
    InvarianceClass,
    MixedDistillation,
}

impl TassadarCompiledDistillationMode {
    /// Returns the stable mode label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullTrace => "full_trace",
            Self::IoOnly => "io_only",
            Self::PartialState => "partial_state",
            Self::InvarianceClass => "invariance_class",
            Self::MixedDistillation => "mixed_distillation",
        }
    }
}

/// Workload family tracked by the distillation lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledDistillationWorkloadFamily {
    KernelArithmetic,
    ClrsWasmShortestPath,
    HungarianMatching,
    SudokuSearch,
}

impl TassadarCompiledDistillationWorkloadFamily {
    /// Returns the stable workload-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::KernelArithmetic => "kernel_arithmetic",
            Self::ClrsWasmShortestPath => "clrs_wasm_shortest_path",
            Self::HungarianMatching => "hungarian_matching",
            Self::SudokuSearch => "sudoku_search",
        }
    }
}

/// Invariance class emitted by the compiled/reference authority lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledDistillationInvarianceClass {
    OutputEquivalence,
    ProgressMonotonicity,
    StateDigestEquivalence,
    SelectionStability,
}

impl TassadarCompiledDistillationInvarianceClass {
    /// Returns the stable invariance-class label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OutputEquivalence => "output_equivalence",
            Self::ProgressMonotonicity => "progress_monotonicity",
            Self::StateDigestEquivalence => "state_digest_equivalence",
            Self::SelectionStability => "selection_stability",
        }
    }
}

/// One workload row in the compiled distillation contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationWorkloadRow {
    pub workload_family: TassadarCompiledDistillationWorkloadFamily,
    pub trace_abi_profile_id: String,
    pub supported_modes: Vec<TassadarCompiledDistillationMode>,
    pub invariance_classes: Vec<TassadarCompiledDistillationInvarianceClass>,
    pub compiled_anchor_refs: Vec<String>,
    pub claim_boundary: String,
}

impl TassadarCompiledDistillationWorkloadRow {
    fn validate(&self) -> Result<(), TassadarCompiledDistillationContractError> {
        if self.trace_abi_profile_id.trim().is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingTraceAbiProfileId {
                workload_family: self.workload_family,
            });
        }
        if self.supported_modes.is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingSupportedModes {
                workload_family: self.workload_family,
            });
        }
        if self.invariance_classes.is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingInvarianceClasses {
                workload_family: self.workload_family,
            });
        }
        if self.compiled_anchor_refs.is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingCompiledAnchors {
                workload_family: self.workload_family,
            });
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingClaimBoundary {
                workload_family: self.workload_family,
            });
        }
        Ok(())
    }
}

/// Public contract for compiled-to-learned distillation without full trace lockstep.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationContract {
    pub abi_version: String,
    pub contract_ref: String,
    pub version: String,
    pub supported_modes: Vec<TassadarCompiledDistillationMode>,
    pub workload_rows: Vec<TassadarCompiledDistillationWorkloadRow>,
    pub evaluation_axes: Vec<String>,
    pub target_bundle_ref: String,
    pub training_evidence_bundle_ref: String,
    pub report_ref: String,
    pub summary_report_ref: String,
    pub contract_digest: String,
}

impl TassadarCompiledDistillationContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_COMPILED_DISTILLATION_ABI_VERSION),
            contract_ref: String::from("dataset://openagents/tassadar/compiled_distillation"),
            version: String::from("2026.03.18"),
            supported_modes: vec![
                TassadarCompiledDistillationMode::FullTrace,
                TassadarCompiledDistillationMode::IoOnly,
                TassadarCompiledDistillationMode::PartialState,
                TassadarCompiledDistillationMode::InvarianceClass,
                TassadarCompiledDistillationMode::MixedDistillation,
            ],
            workload_rows: workload_rows(),
            evaluation_axes: vec![
                String::from("final_output_exactness_bps"),
                String::from("later_window_exactness_bps"),
                String::from("held_out_family_exactness_bps"),
                String::from("refusal_rate_bps"),
                String::from("invariance_ablation_delta_bps"),
            ],
            target_bundle_ref: String::from(TASSADAR_COMPILED_DISTILLATION_TARGET_BUNDLE_REF),
            training_evidence_bundle_ref: String::from(
                TASSADAR_COMPILED_DISTILLATION_TRAINING_EVIDENCE_BUNDLE_REF,
            ),
            report_ref: String::from(TASSADAR_COMPILED_DISTILLATION_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_COMPILED_DISTILLATION_SUMMARY_REPORT_REF),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("compiled distillation contract should validate");
        contract.contract_digest =
            stable_digest(b"psionic_tassadar_compiled_distillation_contract|", &contract);
        contract
    }

    /// Validates the compiled distillation contract.
    pub fn validate(&self) -> Result<(), TassadarCompiledDistillationContractError> {
        if self.abi_version != TASSADAR_COMPILED_DISTILLATION_ABI_VERSION {
            return Err(TassadarCompiledDistillationContractError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingContractRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingVersion);
        }
        if self.supported_modes.is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingModes);
        }
        if self.workload_rows.is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingWorkloadRows);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarCompiledDistillationContractError::MissingEvaluationAxes);
        }
        let mut seen_modes = BTreeSet::new();
        for mode in &self.supported_modes {
            if !seen_modes.insert(*mode) {
                return Err(TassadarCompiledDistillationContractError::DuplicateMode {
                    mode: *mode,
                });
            }
        }
        let mut seen_workloads = BTreeSet::new();
        for row in &self.workload_rows {
            row.validate()?;
            if !seen_workloads.insert(row.workload_family) {
                return Err(TassadarCompiledDistillationContractError::DuplicateWorkloadRow {
                    workload_family: row.workload_family,
                });
            }
        }
        Ok(())
    }
}

/// Returns the canonical compiled distillation contract.
#[must_use]
pub fn tassadar_compiled_distillation_contract() -> TassadarCompiledDistillationContract {
    TassadarCompiledDistillationContract::new()
}

/// Compiled distillation contract validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarCompiledDistillationContractError {
    #[error("unsupported compiled distillation ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("compiled distillation contract is missing `contract_ref`")]
    MissingContractRef,
    #[error("compiled distillation contract is missing `version`")]
    MissingVersion,
    #[error("compiled distillation contract must declare modes")]
    MissingModes,
    #[error("compiled distillation contract must declare workload rows")]
    MissingWorkloadRows,
    #[error("compiled distillation contract must declare evaluation axes")]
    MissingEvaluationAxes,
    #[error("compiled distillation contract repeated mode `{mode:?}`")]
    DuplicateMode {
        mode: TassadarCompiledDistillationMode,
    },
    #[error("compiled distillation contract repeated workload `{workload_family:?}`")]
    DuplicateWorkloadRow {
        workload_family: TassadarCompiledDistillationWorkloadFamily,
    },
    #[error("compiled distillation workload `{workload_family:?}` is missing `trace_abi_profile_id`")]
    MissingTraceAbiProfileId {
        workload_family: TassadarCompiledDistillationWorkloadFamily,
    },
    #[error("compiled distillation workload `{workload_family:?}` must declare modes")]
    MissingSupportedModes {
        workload_family: TassadarCompiledDistillationWorkloadFamily,
    },
    #[error("compiled distillation workload `{workload_family:?}` must declare invariance classes")]
    MissingInvarianceClasses {
        workload_family: TassadarCompiledDistillationWorkloadFamily,
    },
    #[error("compiled distillation workload `{workload_family:?}` must declare compiled anchors")]
    MissingCompiledAnchors {
        workload_family: TassadarCompiledDistillationWorkloadFamily,
    },
    #[error("compiled distillation workload `{workload_family:?}` is missing `claim_boundary`")]
    MissingClaimBoundary {
        workload_family: TassadarCompiledDistillationWorkloadFamily,
    },
}

fn workload_rows() -> Vec<TassadarCompiledDistillationWorkloadRow> {
    vec![
        TassadarCompiledDistillationWorkloadRow {
            workload_family: TassadarCompiledDistillationWorkloadFamily::KernelArithmetic,
            trace_abi_profile_id: String::from("tassadar.wasm.core_i32.v2"),
            supported_modes: vec![
                TassadarCompiledDistillationMode::FullTrace,
                TassadarCompiledDistillationMode::IoOnly,
                TassadarCompiledDistillationMode::PartialState,
                TassadarCompiledDistillationMode::InvarianceClass,
                TassadarCompiledDistillationMode::MixedDistillation,
            ],
            invariance_classes: vec![
                TassadarCompiledDistillationInvarianceClass::OutputEquivalence,
                TassadarCompiledDistillationInvarianceClass::ProgressMonotonicity,
            ],
            compiled_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_compile_pipeline_matrix_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"),
            ],
            claim_boundary: String::from(
                "kernel arithmetic remains a bounded compiled/reference anchor for distillation target emission only; it does not imply broad learned closure",
            ),
        },
        TassadarCompiledDistillationWorkloadRow {
            workload_family: TassadarCompiledDistillationWorkloadFamily::ClrsWasmShortestPath,
            trace_abi_profile_id: String::from("tassadar.wasm.article_i32_compute.v1"),
            supported_modes: vec![
                TassadarCompiledDistillationMode::FullTrace,
                TassadarCompiledDistillationMode::IoOnly,
                TassadarCompiledDistillationMode::PartialState,
                TassadarCompiledDistillationMode::InvarianceClass,
                TassadarCompiledDistillationMode::MixedDistillation,
            ],
            invariance_classes: vec![
                TassadarCompiledDistillationInvarianceClass::OutputEquivalence,
                TassadarCompiledDistillationInvarianceClass::StateDigestEquivalence,
                TassadarCompiledDistillationInvarianceClass::ProgressMonotonicity,
            ],
            compiled_anchor_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            )],
            claim_boundary: String::from(
                "the CLRS-to-Wasm row keeps compiled bridge truth and lighter supervision comparable without claiming full CLRS or broad learned transfer closure",
            ),
        },
        TassadarCompiledDistillationWorkloadRow {
            workload_family: TassadarCompiledDistillationWorkloadFamily::HungarianMatching,
            trace_abi_profile_id: String::from("tassadar.wasm.hungarian_v0_matching.v1"),
            supported_modes: vec![
                TassadarCompiledDistillationMode::FullTrace,
                TassadarCompiledDistillationMode::IoOnly,
                TassadarCompiledDistillationMode::PartialState,
                TassadarCompiledDistillationMode::InvarianceClass,
                TassadarCompiledDistillationMode::MixedDistillation,
            ],
            invariance_classes: vec![
                TassadarCompiledDistillationInvarianceClass::OutputEquivalence,
                TassadarCompiledDistillationInvarianceClass::SelectionStability,
                TassadarCompiledDistillationInvarianceClass::StateDigestEquivalence,
            ],
            compiled_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"),
            ],
            claim_boundary: String::from(
                "matching-family distillation remains bounded to the current Hungarian witnesses and keeps weaker supervision separate from compiled exactness",
            ),
        },
        TassadarCompiledDistillationWorkloadRow {
            workload_family: TassadarCompiledDistillationWorkloadFamily::SudokuSearch,
            trace_abi_profile_id: String::from("tassadar.wasm.sudoku_v0_search.v1"),
            supported_modes: vec![
                TassadarCompiledDistillationMode::FullTrace,
                TassadarCompiledDistillationMode::IoOnly,
                TassadarCompiledDistillationMode::PartialState,
                TassadarCompiledDistillationMode::InvarianceClass,
                TassadarCompiledDistillationMode::MixedDistillation,
            ],
            invariance_classes: vec![
                TassadarCompiledDistillationInvarianceClass::OutputEquivalence,
                TassadarCompiledDistillationInvarianceClass::ProgressMonotonicity,
                TassadarCompiledDistillationInvarianceClass::SelectionStability,
            ],
            compiled_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"),
            ],
            claim_boundary: String::from(
                "Sudoku distillation remains a bounded search-family row with explicit refusal when lighter supervision cannot preserve the declared search semantics",
            ),
        },
    ]
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
        tassadar_compiled_distillation_contract, TassadarCompiledDistillationMode,
        TassadarCompiledDistillationWorkloadFamily,
    };

    #[test]
    fn compiled_distillation_contract_is_machine_legible() {
        let contract = tassadar_compiled_distillation_contract();

        assert_eq!(contract.abi_version, "psionic.tassadar.compiled_distillation.v1");
        assert_eq!(contract.workload_rows.len(), 4);
        assert!(contract
            .supported_modes
            .contains(&TassadarCompiledDistillationMode::MixedDistillation));
        assert!(contract.workload_rows.iter().any(|row| {
            row.workload_family == TassadarCompiledDistillationWorkloadFamily::SudokuSearch
                && row.trace_abi_profile_id == "tassadar.wasm.sudoku_v0_search.v1"
        }));
        assert!(!contract.contract_digest.is_empty());
    }
}
