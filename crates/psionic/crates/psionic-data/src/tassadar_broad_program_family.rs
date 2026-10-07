use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_BROAD_PROGRAM_FAMILY_SUITE_ABI_VERSION: &str =
    "psionic.tassadar.broad_program_family_suite.v1";
pub const TASSADAR_BROAD_PROGRAM_FAMILY_SUITE_REF: &str =
    "dataset://openagents/tassadar/broad_program_family_suite";
pub const TASSADAR_BROAD_PROGRAM_FAMILY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json";
pub const TASSADAR_BROAD_PROGRAM_FAMILY_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_architecture_bakeoff_summary.json";

/// Workload family carried by the broadened architecture matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarBroadProgramFamily {
    ArithmeticMultiOperand,
    ClrsShortestPath,
    SudokuBacktrackingSearch,
    ModuleScaleWasmLoop,
    LongHorizonControl,
    CheckpointedResumableProgram,
    LinkedProgramBundle,
    ImportMediatedProcess,
    StatefulProcessLoop,
}

impl TassadarBroadProgramFamily {
    /// Returns the stable workload-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ArithmeticMultiOperand => "arithmetic_multi_operand",
            Self::ClrsShortestPath => "clrs_shortest_path",
            Self::SudokuBacktrackingSearch => "sudoku_backtracking_search",
            Self::ModuleScaleWasmLoop => "module_scale_wasm_loop",
            Self::LongHorizonControl => "long_horizon_control",
            Self::CheckpointedResumableProgram => "checkpointed_resumable_program",
            Self::LinkedProgramBundle => "linked_program_bundle",
            Self::ImportMediatedProcess => "import_mediated_process",
            Self::StatefulProcessLoop => "stateful_process_loop",
        }
    }
}

/// One family row in the broadened program-family suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadProgramFamilyRow {
    pub workload_family: TassadarBroadProgramFamily,
    pub authority_refs: Vec<String>,
    pub claim_boundary: String,
}

impl TassadarBroadProgramFamilyRow {
    fn validate(&self) -> Result<(), TassadarBroadProgramFamilySuiteError> {
        if self.authority_refs.is_empty() {
            return Err(TassadarBroadProgramFamilySuiteError::MissingAuthorityRefs {
                workload_family: self.workload_family,
            });
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(TassadarBroadProgramFamilySuiteError::MissingClaimBoundary {
                workload_family: self.workload_family,
            });
        }
        Ok(())
    }
}

/// Public contract for the broadened architecture bakeoff workload suite.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarBroadProgramFamilySuiteContract {
    pub abi_version: String,
    pub suite_ref: String,
    pub version: String,
    pub workload_rows: Vec<TassadarBroadProgramFamilyRow>,
    pub evaluation_axes: Vec<String>,
    pub report_ref: String,
    pub summary_report_ref: String,
    pub contract_digest: String,
}

impl TassadarBroadProgramFamilySuiteContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_BROAD_PROGRAM_FAMILY_SUITE_ABI_VERSION),
            suite_ref: String::from(TASSADAR_BROAD_PROGRAM_FAMILY_SUITE_REF),
            version: String::from("2026.03.18"),
            workload_rows: workload_rows(),
            evaluation_axes: vec![
                String::from("exactness_bps"),
                String::from("cost_score_bps"),
                String::from("stability_bps"),
                String::from("ownership_posture"),
                String::from("refusal_posture"),
                String::from("portability_posture"),
            ],
            report_ref: String::from(TASSADAR_BROAD_PROGRAM_FAMILY_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_BROAD_PROGRAM_FAMILY_SUMMARY_REF),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("broad program-family suite contract should validate");
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_broad_program_family_suite_contract|",
            &contract,
        );
        contract
    }

    /// Validates the suite contract.
    pub fn validate(&self) -> Result<(), TassadarBroadProgramFamilySuiteError> {
        if self.abi_version != TASSADAR_BROAD_PROGRAM_FAMILY_SUITE_ABI_VERSION {
            return Err(TassadarBroadProgramFamilySuiteError::UnsupportedAbiVersion {
                abi_version: self.abi_version.clone(),
            });
        }
        if self.suite_ref.trim().is_empty() {
            return Err(TassadarBroadProgramFamilySuiteError::MissingSuiteRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarBroadProgramFamilySuiteError::MissingVersion);
        }
        if self.workload_rows.is_empty() {
            return Err(TassadarBroadProgramFamilySuiteError::MissingWorkloadRows);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarBroadProgramFamilySuiteError::MissingEvaluationAxes);
        }
        if self.report_ref.trim().is_empty() || self.summary_report_ref.trim().is_empty() {
            return Err(TassadarBroadProgramFamilySuiteError::MissingReportRefs);
        }

        let mut seen_workloads = BTreeSet::new();
        for workload_row in &self.workload_rows {
            workload_row.validate()?;
            if !seen_workloads.insert(workload_row.workload_family) {
                return Err(TassadarBroadProgramFamilySuiteError::DuplicateWorkloadFamily {
                    workload_family: workload_row.workload_family,
                });
            }
        }
        Ok(())
    }

    /// Returns the stable workload-family ids in contract order.
    #[must_use]
    pub fn workload_family_ids(&self) -> Vec<String> {
        self.workload_rows
            .iter()
            .map(|row| row.workload_family.as_str().to_string())
            .collect()
    }
}

/// Public suite-contract error for the broadened program-family lane.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarBroadProgramFamilySuiteError {
    #[error("unsupported broad-program-family ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("broad-program-family suite contract is missing `suite_ref`")]
    MissingSuiteRef,
    #[error("broad-program-family suite contract is missing `version`")]
    MissingVersion,
    #[error("broad-program-family suite contract is missing `workload_rows`")]
    MissingWorkloadRows,
    #[error("broad-program-family suite contract has duplicate workload family `{workload_family:?}`")]
    DuplicateWorkloadFamily {
        workload_family: TassadarBroadProgramFamily,
    },
    #[error("broad-program-family suite contract is missing authority refs for `{workload_family:?}`")]
    MissingAuthorityRefs {
        workload_family: TassadarBroadProgramFamily,
    },
    #[error("broad-program-family suite contract is missing claim boundary for `{workload_family:?}`")]
    MissingClaimBoundary {
        workload_family: TassadarBroadProgramFamily,
    },
    #[error("broad-program-family suite contract is missing `evaluation_axes`")]
    MissingEvaluationAxes,
    #[error("broad-program-family suite contract is missing report refs")]
    MissingReportRefs,
}

/// Returns the canonical broadened program-family suite contract.
#[must_use]
pub fn tassadar_broad_program_family_suite_contract() -> TassadarBroadProgramFamilySuiteContract {
    TassadarBroadProgramFamilySuiteContract::new()
}

fn workload_rows() -> Vec<TassadarBroadProgramFamilyRow> {
    vec![
        row(
            TassadarBroadProgramFamily::ArithmeticMultiOperand,
            &["fixtures/tassadar/reports/tassadar_generalized_abi_family_report.json"],
            "arithmetic multi-operand programs remain the bounded arithmetic anchor for the bakeoff rather than a proxy for broader program closure",
        ),
        row(
            TassadarBroadProgramFamily::ClrsShortestPath,
            &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "CLRS shortest-path remains the graph-frontier program family in the broadened suite",
        ),
        row(
            TassadarBroadProgramFamily::SudokuBacktrackingSearch,
            &["fixtures/tassadar/reports/tassadar_search_native_executor_report.json"],
            "Sudoku backtracking remains the bounded search-heavy family in the broadened suite",
        ),
        row(
            TassadarBroadProgramFamily::ModuleScaleWasmLoop,
            &["fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json"],
            "module-scale Wasm loop remains the current bounded process-style module family rather than a stand-in for arbitrary Wasm",
        ),
        row(
            TassadarBroadProgramFamily::LongHorizonControl,
            &["fixtures/tassadar/reports/tassadar_article_runtime_closeout_report.json"],
            "long-horizon control remains the current long-running direct-execution family rather than a claim about generic million-step closure",
        ),
        row(
            TassadarBroadProgramFamily::CheckpointedResumableProgram,
            &["fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json"],
            "checkpointed resumable programs are anchored to the exact checkpoint lane and do not imply arbitrary resumable computation",
        ),
        row(
            TassadarBroadProgramFamily::LinkedProgramBundle,
            &[
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
                "fixtures/tassadar/reports/tassadar_module_link_eval_report.json",
            ],
            "linked program bundles stay explicit as bounded linked-module evidence instead of being read as general library or runtime-support closure",
        ),
        row(
            TassadarBroadProgramFamily::ImportMediatedProcess,
            &[
                "fixtures/tassadar/reports/tassadar_effect_taxonomy_report.json",
                "fixtures/tassadar/reports/tassadar_composite_routing_report.json",
            ],
            "import-mediated processes stay effect-taxonomy-bound and route-policy-bound instead of collapsing into generic host behavior",
        ),
        row(
            TassadarBroadProgramFamily::StatefulProcessLoop,
            &[
                "fixtures/tassadar/reports/tassadar_working_memory_tier_summary.json",
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            ],
            "stateful process loops stay benchmarked as bounded working-memory plus checkpoint evidence rather than generic deployment-grade stateful closure",
        ),
    ]
}

fn row(
    workload_family: TassadarBroadProgramFamily,
    authority_refs: &[&str],
    claim_boundary: &str,
) -> TassadarBroadProgramFamilyRow {
    TassadarBroadProgramFamilyRow {
        workload_family,
        authority_refs: authority_refs.iter().map(|value| String::from(*value)).collect(),
        claim_boundary: String::from(claim_boundary),
    }
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
        TassadarBroadProgramFamily, TASSADAR_BROAD_PROGRAM_FAMILY_REPORT_REF,
        tassadar_broad_program_family_suite_contract,
    };

    #[test]
    fn broad_program_family_suite_contract_is_machine_legible() {
        let contract = tassadar_broad_program_family_suite_contract();

        assert_eq!(contract.workload_rows.len(), 9);
        assert_eq!(contract.evaluation_axes.len(), 6);
        assert_eq!(contract.report_ref, TASSADAR_BROAD_PROGRAM_FAMILY_REPORT_REF);
        assert!(contract.workload_rows.iter().any(|row| {
            row.workload_family == TassadarBroadProgramFamily::LinkedProgramBundle
                && row.authority_refs.len() == 2
        }));
        assert!(contract.workload_rows.iter().any(|row| {
            row.workload_family == TassadarBroadProgramFamily::ImportMediatedProcess
        }));
    }
}
