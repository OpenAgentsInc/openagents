use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SEARCH_NATIVE_EXECUTOR_ABI_VERSION: &str =
    "psionic.tassadar.search_native_executor.v1";
pub const TASSADAR_SEARCH_NATIVE_EXECUTOR_CONTRACT_REF: &str =
    "dataset://openagents/tassadar/search_native_executor";
pub const TASSADAR_SEARCH_NATIVE_EXECUTOR_EVIDENCE_BUNDLE_REF: &str = "fixtures/tassadar/runs/tassadar_search_native_executor_v1/search_native_executor_evidence_bundle.json";
pub const TASSADAR_SEARCH_NATIVE_EXECUTOR_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_search_native_executor_runtime_report.json";
pub const TASSADAR_SEARCH_NATIVE_EXECUTOR_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_search_native_executor_report.json";

/// First-class event families admitted by the search-native executor lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSearchNativeEventKind {
    Guess,
    Verify,
    Contradict,
    Backtrack,
    BranchSummary,
    SearchBudget,
}

impl TassadarSearchNativeEventKind {
    /// Returns the stable event label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Guess => "guess",
            Self::Verify => "verify",
            Self::Contradict => "contradict",
            Self::Backtrack => "backtrack",
            Self::BranchSummary => "branch_summary",
            Self::SearchBudget => "search_budget",
        }
    }
}

/// Search-heavy workload family compared by the search-native lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSearchNativeWorkloadFamily {
    SudokuBacktrackingSearch,
    BranchHeavyClrsVariant,
    SearchKernelRecovery,
    VerifierHeavyWorkloadPack,
}

impl TassadarSearchNativeWorkloadFamily {
    /// Returns the stable workload-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SudokuBacktrackingSearch => "sudoku_backtracking_search",
            Self::BranchHeavyClrsVariant => "branch_heavy_clrs_variant",
            Self::SearchKernelRecovery => "search_kernel_recovery",
            Self::VerifierHeavyWorkloadPack => "verifier_heavy_workload_pack",
        }
    }
}

/// One workload row in the public search-native contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeWorkloadRow {
    pub workload_family: TassadarSearchNativeWorkloadFamily,
    pub search_budget_limit: u32,
    pub baseline_refs: Vec<String>,
    pub claim_boundary: String,
}

impl TassadarSearchNativeWorkloadRow {
    fn validate(&self) -> Result<(), TassadarSearchNativeExecutorContractError> {
        if self.search_budget_limit == 0 {
            return Err(
                TassadarSearchNativeExecutorContractError::InvalidSearchBudgetLimit {
                    workload_family: self.workload_family,
                },
            );
        }
        if self.baseline_refs.is_empty() {
            return Err(
                TassadarSearchNativeExecutorContractError::MissingBaselineRefs {
                    workload_family: self.workload_family,
                },
            );
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(
                TassadarSearchNativeExecutorContractError::MissingClaimBoundary {
                    workload_family: self.workload_family,
                },
            );
        }
        Ok(())
    }
}

/// Public contract for the search-native executor family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSearchNativeExecutorContract {
    pub abi_version: String,
    pub contract_ref: String,
    pub version: String,
    pub event_kinds: Vec<TassadarSearchNativeEventKind>,
    pub state_surfaces: Vec<String>,
    pub workload_rows: Vec<TassadarSearchNativeWorkloadRow>,
    pub evaluation_axes: Vec<String>,
    pub evidence_bundle_ref: String,
    pub runtime_report_ref: String,
    pub report_ref: String,
    pub contract_digest: String,
}

impl TassadarSearchNativeExecutorContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_ABI_VERSION),
            contract_ref: String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_CONTRACT_REF),
            version: String::from("2026.03.18"),
            event_kinds: vec![
                TassadarSearchNativeEventKind::Guess,
                TassadarSearchNativeEventKind::Verify,
                TassadarSearchNativeEventKind::Contradict,
                TassadarSearchNativeEventKind::Backtrack,
                TassadarSearchNativeEventKind::BranchSummary,
                TassadarSearchNativeEventKind::SearchBudget,
            ],
            state_surfaces: vec![
                String::from("guess_state"),
                String::from("verifier_state"),
                String::from("contradiction_state"),
                String::from("backtrack_state"),
                String::from("branch_summary_state"),
                String::from("search_budget_state"),
            ],
            workload_rows: workload_rows(),
            evaluation_axes: vec![
                String::from("exactness_bps"),
                String::from("recovery_quality_bps"),
                String::from("guess_efficiency_bps"),
                String::from("search_budget_utilization_bps"),
                String::from("refusal_cleanliness_bps"),
            ],
            evidence_bundle_ref: String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_EVIDENCE_BUNDLE_REF),
            runtime_report_ref: String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_RUNTIME_REPORT_REF),
            report_ref: String::from(TASSADAR_SEARCH_NATIVE_EXECUTOR_REPORT_REF),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("search-native executor contract should validate");
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_search_native_executor_contract|",
            &contract,
        );
        contract
    }

    /// Validates the public search-native contract.
    pub fn validate(&self) -> Result<(), TassadarSearchNativeExecutorContractError> {
        if self.abi_version != TASSADAR_SEARCH_NATIVE_EXECUTOR_ABI_VERSION {
            return Err(
                TassadarSearchNativeExecutorContractError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarSearchNativeExecutorContractError::MissingContractRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarSearchNativeExecutorContractError::MissingVersion);
        }
        if self.event_kinds.is_empty() {
            return Err(TassadarSearchNativeExecutorContractError::MissingEventKinds);
        }
        if self.state_surfaces.is_empty() {
            return Err(TassadarSearchNativeExecutorContractError::MissingStateSurfaces);
        }
        if self.workload_rows.is_empty() {
            return Err(TassadarSearchNativeExecutorContractError::MissingWorkloadRows);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarSearchNativeExecutorContractError::MissingEvaluationAxes);
        }
        if self.evidence_bundle_ref.trim().is_empty()
            || self.runtime_report_ref.trim().is_empty()
            || self.report_ref.trim().is_empty()
        {
            return Err(TassadarSearchNativeExecutorContractError::MissingReportRefs);
        }

        let mut seen_event_kinds = BTreeSet::new();
        for event_kind in &self.event_kinds {
            if !seen_event_kinds.insert(*event_kind) {
                return Err(
                    TassadarSearchNativeExecutorContractError::DuplicateEventKind {
                        event_kind: *event_kind,
                    },
                );
            }
        }
        let mut seen_state_surfaces = BTreeSet::new();
        for state_surface in &self.state_surfaces {
            if state_surface.trim().is_empty() {
                return Err(TassadarSearchNativeExecutorContractError::MissingStateSurface);
            }
            if !seen_state_surfaces.insert(state_surface.clone()) {
                return Err(
                    TassadarSearchNativeExecutorContractError::DuplicateStateSurface {
                        state_surface: state_surface.clone(),
                    },
                );
            }
        }
        let mut seen_workloads = BTreeSet::new();
        for workload_row in &self.workload_rows {
            workload_row.validate()?;
            if !seen_workloads.insert(workload_row.workload_family) {
                return Err(
                    TassadarSearchNativeExecutorContractError::DuplicateWorkloadFamily {
                        workload_family: workload_row.workload_family,
                    },
                );
            }
        }
        Ok(())
    }
}

/// Public contract error for the search-native executor lane.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarSearchNativeExecutorContractError {
    #[error("unsupported search-native executor ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("search-native executor contract is missing `contract_ref`")]
    MissingContractRef,
    #[error("search-native executor contract is missing `version`")]
    MissingVersion,
    #[error("search-native executor contract is missing `event_kinds`")]
    MissingEventKinds,
    #[error("search-native executor contract has duplicate event kind `{event_kind:?}`")]
    DuplicateEventKind {
        event_kind: TassadarSearchNativeEventKind,
    },
    #[error("search-native executor contract is missing `state_surfaces`")]
    MissingStateSurfaces,
    #[error("search-native executor contract contains an empty state surface")]
    MissingStateSurface,
    #[error("search-native executor contract has duplicate state surface `{state_surface}`")]
    DuplicateStateSurface { state_surface: String },
    #[error("search-native executor contract is missing `workload_rows`")]
    MissingWorkloadRows,
    #[error("search-native executor contract has duplicate workload family `{workload_family:?}`")]
    DuplicateWorkloadFamily {
        workload_family: TassadarSearchNativeWorkloadFamily,
    },
    #[error("search-native executor contract is missing `evaluation_axes`")]
    MissingEvaluationAxes,
    #[error("search-native executor contract is missing one or more report refs")]
    MissingReportRefs,
    #[error("search-native workload `{workload_family:?}` has invalid `search_budget_limit=0`")]
    InvalidSearchBudgetLimit {
        workload_family: TassadarSearchNativeWorkloadFamily,
    },
    #[error("search-native workload `{workload_family:?}` is missing baseline refs")]
    MissingBaselineRefs {
        workload_family: TassadarSearchNativeWorkloadFamily,
    },
    #[error("search-native workload `{workload_family:?}` is missing claim boundary")]
    MissingClaimBoundary {
        workload_family: TassadarSearchNativeWorkloadFamily,
    },
}

/// Returns the canonical public search-native contract.
#[must_use]
pub fn tassadar_search_native_executor_contract() -> TassadarSearchNativeExecutorContract {
    TassadarSearchNativeExecutorContract::new()
}

fn workload_rows() -> Vec<TassadarSearchNativeWorkloadRow> {
    vec![
        TassadarSearchNativeWorkloadRow {
            workload_family: TassadarSearchNativeWorkloadFamily::SudokuBacktrackingSearch,
            search_budget_limit: 12,
            baseline_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json"),
            ],
            claim_boundary: String::from(
                "search-native Sudoku rows stay benchmark-bound and do not imply general solver closure outside the seeded search family",
            ),
        },
        TassadarSearchNativeWorkloadRow {
            workload_family: TassadarSearchNativeWorkloadFamily::BranchHeavyClrsVariant,
            search_budget_limit: 10,
            baseline_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_architecture_bakeoff_report.json"),
            ],
            claim_boundary: String::from(
                "branch-heavy CLRS rows keep graph-search gains benchmark-bound instead of treating them as general CLRS ownership",
            ),
        },
        TassadarSearchNativeWorkloadRow {
            workload_family: TassadarSearchNativeWorkloadFamily::SearchKernelRecovery,
            search_budget_limit: 8,
            baseline_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_shared_primitive_transfer_report.json",
                ),
            ],
            claim_boundary: String::from(
                "search-kernel rows keep guess and recovery improvements explicit without widening the current search trace family into broad executor promotion",
            ),
        },
        TassadarSearchNativeWorkloadRow {
            workload_family: TassadarSearchNativeWorkloadFamily::VerifierHeavyWorkloadPack,
            search_budget_limit: 10,
            baseline_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_latency_evidence_tradeoff_report.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                ),
            ],
            claim_boundary: String::from(
                "verifier-heavy pack rows keep search-budget refusal explicit and do not turn one refusal-clean path into broad validator-heavy closure",
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
        TassadarSearchNativeEventKind, TassadarSearchNativeWorkloadFamily,
        tassadar_search_native_executor_contract,
    };

    #[test]
    fn search_native_executor_contract_is_machine_legible() {
        let contract = tassadar_search_native_executor_contract();

        assert_eq!(contract.event_kinds.len(), 6);
        assert!(
            contract
                .event_kinds
                .contains(&TassadarSearchNativeEventKind::BranchSummary)
        );
        assert!(contract.workload_rows.iter().any(|row| {
            row.workload_family == TassadarSearchNativeWorkloadFamily::VerifierHeavyWorkloadPack
                && row.search_budget_limit == 10
        }));
        assert!(!contract.contract_digest.is_empty());
    }
}
