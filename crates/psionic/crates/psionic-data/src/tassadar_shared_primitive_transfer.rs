use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_ABI_VERSION: &str =
    "psionic.tassadar.shared_primitive_transfer.v1";
pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_shared_primitive_transfer_v1/shared_primitive_transfer_evidence_bundle.json";
pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_primitive_transfer_report.json";
pub const TASSADAR_SHARED_PRIMITIVE_TRANSFER_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_primitive_transfer_summary.json";

/// Shared primitive family tracked by the transfer substrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedPrimitiveKind {
    ReachabilityExpand,
    RelaxState,
    CompareCandidates,
    SelectCandidate,
    MergeState,
    BoundedBacktrack,
}

impl TassadarSharedPrimitiveKind {
    /// Returns the stable primitive-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReachabilityExpand => "reachability_expand",
            Self::RelaxState => "relax_state",
            Self::CompareCandidates => "compare_candidates",
            Self::SelectCandidate => "select_candidate",
            Self::MergeState => "merge_state",
            Self::BoundedBacktrack => "bounded_backtrack",
        }
    }
}

/// Algorithm family reused by the shared primitive transfer substrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedPrimitiveAlgorithmFamily {
    SortMerge,
    ClrsShortestPath,
    ClrsWasmShortestPath,
    HungarianMatching,
    SudokuSearch,
    VerifierSearchKernel,
}

impl TassadarSharedPrimitiveAlgorithmFamily {
    /// Returns the stable algorithm-family label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SortMerge => "sort_merge",
            Self::ClrsShortestPath => "clrs_shortest_path",
            Self::ClrsWasmShortestPath => "clrs_wasm_shortest_path",
            Self::HungarianMatching => "hungarian_matching",
            Self::SudokuSearch => "sudoku_search",
            Self::VerifierSearchKernel => "verifier_search_kernel",
        }
    }
}

/// One public primitive row in the shared transfer substrate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferRow {
    pub primitive_id: String,
    pub primitive_kind: TassadarSharedPrimitiveKind,
    pub summary: String,
    pub supported_algorithm_families: Vec<TassadarSharedPrimitiveAlgorithmFamily>,
    pub related_subroutine_ids: Vec<String>,
    pub compiled_anchor_refs: Vec<String>,
    pub learned_anchor_refs: Vec<String>,
    pub claim_boundary: String,
}

impl TassadarSharedPrimitiveTransferRow {
    fn validate(&self) -> Result<(), TassadarSharedPrimitiveTransferContractError> {
        if self.primitive_id.trim().is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingPrimitiveId);
        }
        if self.summary.trim().is_empty() {
            return Err(
                TassadarSharedPrimitiveTransferContractError::MissingPrimitiveSummary {
                    primitive_id: self.primitive_id.clone(),
                },
            );
        }
        if self.supported_algorithm_families.is_empty() {
            return Err(
                TassadarSharedPrimitiveTransferContractError::MissingSupportedAlgorithms {
                    primitive_id: self.primitive_id.clone(),
                },
            );
        }
        if self.compiled_anchor_refs.is_empty() {
            return Err(
                TassadarSharedPrimitiveTransferContractError::MissingCompiledAnchors {
                    primitive_id: self.primitive_id.clone(),
                },
            );
        }
        if self.learned_anchor_refs.is_empty() {
            return Err(
                TassadarSharedPrimitiveTransferContractError::MissingLearnedAnchors {
                    primitive_id: self.primitive_id.clone(),
                },
            );
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(
                TassadarSharedPrimitiveTransferContractError::MissingClaimBoundary {
                    primitive_id: self.primitive_id.clone(),
                },
            );
        }
        let mut seen_algorithms = BTreeSet::new();
        for family in &self.supported_algorithm_families {
            if !seen_algorithms.insert(*family) {
                return Err(
                    TassadarSharedPrimitiveTransferContractError::DuplicateSupportedAlgorithm {
                        primitive_id: self.primitive_id.clone(),
                        algorithm_family: *family,
                    },
                );
            }
        }
        Ok(())
    }
}

/// Public contract for the shared primitive transfer substrate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferContract {
    pub abi_version: String,
    pub contract_ref: String,
    pub version: String,
    pub source_library_refs: Vec<String>,
    pub algorithm_families: Vec<TassadarSharedPrimitiveAlgorithmFamily>,
    pub evaluation_axes: Vec<String>,
    pub primitives: Vec<TassadarSharedPrimitiveTransferRow>,
    pub evidence_bundle_ref: String,
    pub report_ref: String,
    pub summary_report_ref: String,
    pub contract_digest: String,
}

impl TassadarSharedPrimitiveTransferContract {
    fn new() -> Self {
        let mut contract = Self {
            abi_version: String::from(TASSADAR_SHARED_PRIMITIVE_TRANSFER_ABI_VERSION),
            contract_ref: String::from("dataset://openagents/tassadar/shared_primitive_transfer"),
            version: String::from("2026.03.18"),
            source_library_refs: vec![
                String::from("model-family://openagents/tassadar/executor_subroutine_library"),
                String::from(
                    "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
                String::from(
                    "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                ),
            ],
            algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
                TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
                TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            ],
            evaluation_axes: vec![
                String::from("primitive_reuse_bps"),
                String::from("final_task_exactness_bps"),
                String::from("composition_gap_bps"),
                String::from("zero_shot_transfer"),
                String::from("few_shot_transfer"),
                String::from("primitive_ablation_delta_bps"),
            ],
            primitives: primitive_rows(),
            evidence_bundle_ref: String::from(
                TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF,
            ),
            report_ref: String::from(TASSADAR_SHARED_PRIMITIVE_TRANSFER_REPORT_REF),
            summary_report_ref: String::from(TASSADAR_SHARED_PRIMITIVE_TRANSFER_SUMMARY_REPORT_REF),
            contract_digest: String::new(),
        };
        contract
            .validate()
            .expect("shared primitive transfer contract should validate");
        contract.contract_digest = stable_digest(
            b"psionic_tassadar_shared_primitive_transfer_contract|",
            &contract,
        );
        contract
    }

    /// Validates the transfer contract.
    pub fn validate(&self) -> Result<(), TassadarSharedPrimitiveTransferContractError> {
        if self.abi_version != TASSADAR_SHARED_PRIMITIVE_TRANSFER_ABI_VERSION {
            return Err(
                TassadarSharedPrimitiveTransferContractError::UnsupportedAbiVersion {
                    abi_version: self.abi_version.clone(),
                },
            );
        }
        if self.contract_ref.trim().is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingContractRef);
        }
        if self.version.trim().is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingVersion);
        }
        if self.source_library_refs.is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingSourceRefs);
        }
        if self.algorithm_families.is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingAlgorithms);
        }
        if self.evaluation_axes.is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingEvaluationAxes);
        }
        if self.primitives.is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingPrimitives);
        }
        if self.evidence_bundle_ref.trim().is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingEvidenceBundleRef);
        }
        if self.report_ref.trim().is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingReportRef);
        }
        if self.summary_report_ref.trim().is_empty() {
            return Err(TassadarSharedPrimitiveTransferContractError::MissingSummaryReportRef);
        }

        let mut seen_algorithms = BTreeSet::new();
        for family in &self.algorithm_families {
            if !seen_algorithms.insert(*family) {
                return Err(
                    TassadarSharedPrimitiveTransferContractError::DuplicateAlgorithmFamily {
                        algorithm_family: *family,
                    },
                );
            }
        }
        let mut seen_axes = BTreeSet::new();
        for axis in &self.evaluation_axes {
            if axis.trim().is_empty() {
                return Err(TassadarSharedPrimitiveTransferContractError::MissingEvaluationAxis);
            }
            if !seen_axes.insert(axis.clone()) {
                return Err(
                    TassadarSharedPrimitiveTransferContractError::DuplicateEvaluationAxis {
                        axis: axis.clone(),
                    },
                );
            }
        }
        let mut seen_primitives = BTreeSet::new();
        for primitive in &self.primitives {
            primitive.validate()?;
            if !seen_primitives.insert(primitive.primitive_id.clone()) {
                return Err(
                    TassadarSharedPrimitiveTransferContractError::DuplicatePrimitiveId {
                        primitive_id: primitive.primitive_id.clone(),
                    },
                );
            }
        }
        Ok(())
    }
}

/// Returns the canonical shared primitive transfer contract.
#[must_use]
pub fn tassadar_shared_primitive_transfer_contract() -> TassadarSharedPrimitiveTransferContract {
    TassadarSharedPrimitiveTransferContract::new()
}

/// Contract validation failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum TassadarSharedPrimitiveTransferContractError {
    #[error("unsupported shared primitive transfer ABI version `{abi_version}`")]
    UnsupportedAbiVersion { abi_version: String },
    #[error("shared primitive transfer contract is missing `contract_ref`")]
    MissingContractRef,
    #[error("shared primitive transfer contract is missing `version`")]
    MissingVersion,
    #[error("shared primitive transfer contract must declare source library refs")]
    MissingSourceRefs,
    #[error("shared primitive transfer contract must declare algorithm families")]
    MissingAlgorithms,
    #[error("shared primitive transfer contract must declare evaluation axes")]
    MissingEvaluationAxes,
    #[error("shared primitive transfer contract contains an empty evaluation axis")]
    MissingEvaluationAxis,
    #[error("shared primitive transfer contract must declare primitive rows")]
    MissingPrimitives,
    #[error("shared primitive transfer contract is missing `evidence_bundle_ref`")]
    MissingEvidenceBundleRef,
    #[error("shared primitive transfer contract is missing `report_ref`")]
    MissingReportRef,
    #[error("shared primitive transfer contract is missing `summary_report_ref`")]
    MissingSummaryReportRef,
    #[error("shared primitive transfer contract repeated algorithm family `{algorithm_family:?}`")]
    DuplicateAlgorithmFamily {
        algorithm_family: TassadarSharedPrimitiveAlgorithmFamily,
    },
    #[error("shared primitive transfer contract repeated evaluation axis `{axis}`")]
    DuplicateEvaluationAxis { axis: String },
    #[error("shared primitive transfer contract repeated primitive id `{primitive_id}`")]
    DuplicatePrimitiveId { primitive_id: String },
    #[error("shared primitive transfer row is missing `primitive_id`")]
    MissingPrimitiveId,
    #[error("shared primitive transfer row `{primitive_id}` is missing `summary`")]
    MissingPrimitiveSummary { primitive_id: String },
    #[error("shared primitive transfer row `{primitive_id}` must declare algorithms")]
    MissingSupportedAlgorithms { primitive_id: String },
    #[error("shared primitive transfer row `{primitive_id}` must declare compiled anchors")]
    MissingCompiledAnchors { primitive_id: String },
    #[error("shared primitive transfer row `{primitive_id}` must declare learned anchors")]
    MissingLearnedAnchors { primitive_id: String },
    #[error("shared primitive transfer row `{primitive_id}` is missing `claim_boundary`")]
    MissingClaimBoundary { primitive_id: String },
    #[error(
        "shared primitive transfer row `{primitive_id}` repeated algorithm family `{algorithm_family:?}`"
    )]
    DuplicateSupportedAlgorithm {
        primitive_id: String,
        algorithm_family: TassadarSharedPrimitiveAlgorithmFamily,
    },
}

fn primitive_rows() -> Vec<TassadarSharedPrimitiveTransferRow> {
    vec![
        TassadarSharedPrimitiveTransferRow {
            primitive_id: String::from("tassadar.primitive.reachability_expand.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::ReachabilityExpand,
            summary: String::from(
                "advance or expand the bounded reachable frontier without collapsing later composition steps into the same target",
            ),
            supported_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
                TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            ],
            related_subroutine_ids: vec![String::from(
                "tassadar.subroutine.advance_cursor.v1",
            )],
            compiled_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"),
            ],
            learned_anchor_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
                ),
                String::from(
                    "fixtures/tassadar/reports/tassadar_shared_depth_architecture_report.json",
                ),
            ],
            claim_boundary: String::from(
                "names one bounded frontier-expansion primitive family shared across declared algorithm families only; it does not imply full executor closure or global graph reasoning completeness",
            ),
        },
        TassadarSharedPrimitiveTransferRow {
            primitive_id: String::from("tassadar.primitive.relax_state.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::RelaxState,
            summary: String::from(
                "apply one bounded score or distance relaxation update while keeping final composition truth separate",
            ),
            supported_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
            ],
            related_subroutine_ids: vec![
                String::from("tassadar.subroutine.compare_candidates.v1"),
                String::from("tassadar.subroutine.commit_update.v1"),
            ],
            compiled_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"),
            ],
            learned_anchor_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_learnability_gap_report.json"),
            ],
            claim_boundary: String::from(
                "keeps one bounded relaxation primitive family reusable across declared shortest-path and matching families only; it does not collapse compiled exactness and learned transfer into one claim",
            ),
        },
        TassadarSharedPrimitiveTransferRow {
            primitive_id: String::from("tassadar.primitive.compare_candidates.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::CompareCandidates,
            summary: String::from(
                "compare two bounded candidate states and surface which branch or assignment should survive",
            ),
            supported_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
            ],
            related_subroutine_ids: vec![String::from(
                "tassadar.subroutine.compare_candidates.v1",
            )],
            compiled_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"),
            ],
            learned_anchor_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
            )],
            claim_boundary: String::from(
                "publishes one bounded comparison primitive shared across the declared algorithm families only; it does not claim arbitrary search, ranking, or dynamic-programming transfer",
            ),
        },
        TassadarSharedPrimitiveTransferRow {
            primitive_id: String::from("tassadar.primitive.select_candidate.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::SelectCandidate,
            summary: String::from(
                "select one bounded next action or surviving candidate after the compare or prune stage",
            ),
            supported_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
                TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
                TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            ],
            related_subroutine_ids: vec![
                String::from("tassadar.subroutine.commit_update.v1"),
                String::from("tassadar.subroutine.prune_candidates.v1"),
            ],
            compiled_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"),
            ],
            learned_anchor_refs: vec![
                String::from(
                    "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
                ),
                String::from("fixtures/tassadar/reports/tassadar_learnability_gap_report.json"),
            ],
            claim_boundary: String::from(
                "keeps one bounded candidate-selection primitive explicit across declared sort, matching, and search families only; it does not widen served decision authority or arbitrary solver closure",
            ),
        },
        TassadarSharedPrimitiveTransferRow {
            primitive_id: String::from("tassadar.primitive.merge_state.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::MergeState,
            summary: String::from(
                "merge two bounded partial states or frontier slices into one next-step working state",
            ),
            supported_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
            ],
            related_subroutine_ids: vec![String::from(
                "tassadar.subroutine.commit_update.v1",
            )],
            compiled_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"),
            ],
            learned_anchor_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
            )],
            claim_boundary: String::from(
                "keeps bounded merge-style state composition explicit across the declared families only; it does not imply arbitrary multi-structure composition or module-scale learned exactness",
            ),
        },
        TassadarSharedPrimitiveTransferRow {
            primitive_id: String::from("tassadar.primitive.bounded_backtrack.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::BoundedBacktrack,
            summary: String::from(
                "retract one bounded failed branch and resume from the last verified state without claiming general solver completeness",
            ),
            supported_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
                TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            ],
            related_subroutine_ids: vec![String::from(
                "tassadar.subroutine.prune_candidates.v1",
            )],
            compiled_anchor_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            )],
            learned_anchor_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_learnability_gap_report.json"),
                String::from(
                    "fixtures/tassadar/reports/tassadar_verifier_guided_search_architecture_report.json",
                ),
            ],
            claim_boundary: String::from(
                "keeps bounded backtrack transfer explicit on the declared search families only; it does not imply arbitrary NP-search closure or promoted solver capability",
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
        tassadar_shared_primitive_transfer_contract, TassadarSharedPrimitiveAlgorithmFamily,
        TassadarSharedPrimitiveKind,
    };

    #[test]
    fn shared_primitive_transfer_contract_is_machine_legible() {
        let contract = tassadar_shared_primitive_transfer_contract();

        assert_eq!(
            contract.abi_version,
            "psionic.tassadar.shared_primitive_transfer.v1"
        );
        assert_eq!(contract.primitives.len(), 6);
        assert!(contract
            .algorithm_families
            .contains(&TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath));
        assert!(contract.primitives.iter().any(|primitive| {
            primitive.primitive_kind == TassadarSharedPrimitiveKind::BoundedBacktrack
                && primitive
                    .supported_algorithm_families
                    .contains(&TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel)
        }));
        assert!(!contract.contract_digest.is_empty());
    }
}
