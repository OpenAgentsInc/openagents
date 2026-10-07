use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    tassadar_shared_primitive_transfer_contract, TassadarSharedPrimitiveAlgorithmFamily,
    TassadarSharedPrimitiveKind, TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF,
};
use psionic_models::tassadar_shared_primitive_transfer_publication;
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

/// Regime used for one held-out transfer case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedPrimitiveTransferRegime {
    ZeroShot,
    FewShot,
}

impl TassadarSharedPrimitiveTransferRegime {
    /// Returns the stable regime label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ZeroShot => "zero_shot",
            Self::FewShot => "few_shot",
        }
    }
}

/// One held-out algorithm transfer receipt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferCase {
    pub case_id: String,
    pub held_out_algorithm_family: TassadarSharedPrimitiveAlgorithmFamily,
    pub regime: TassadarSharedPrimitiveTransferRegime,
    pub reused_primitive_ids: Vec<String>,
    pub primitive_reuse_bps: u32,
    pub final_task_exactness_bps: u32,
    pub evidence_refs: Vec<String>,
    pub detail: String,
}

/// One primitive-ablation receipt over the held-out transfer set.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveAblationReceipt {
    pub primitive_id: String,
    pub primitive_kind: TassadarSharedPrimitiveKind,
    pub held_out_algorithm_families: Vec<TassadarSharedPrimitiveAlgorithmFamily>,
    pub mean_zero_shot_drop_bps: u32,
    pub mean_few_shot_drop_bps: u32,
    pub detail: String,
}

/// Train-side evidence bundle for the shared primitive transfer substrate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedPrimitiveTransferEvidenceBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub contract_digest: String,
    pub publication_digest: String,
    pub transfer_cases: Vec<TassadarSharedPrimitiveTransferCase>,
    pub primitive_ablations: Vec<TassadarSharedPrimitiveAblationReceipt>,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

impl TassadarSharedPrimitiveTransferEvidenceBundle {
    fn new(
        transfer_cases: Vec<TassadarSharedPrimitiveTransferCase>,
        primitive_ablations: Vec<TassadarSharedPrimitiveAblationReceipt>,
    ) -> Self {
        let contract = tassadar_shared_primitive_transfer_contract();
        let publication = tassadar_shared_primitive_transfer_publication();
        let mut bundle = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            bundle_id: String::from("tassadar.shared_primitive_transfer.evidence_bundle.v1"),
            contract_digest: contract.contract_digest,
            publication_digest: publication.publication_digest,
            transfer_cases,
            primitive_ablations,
            claim_boundary: String::from(
                "this bundle records bounded shared primitive transfer evidence over declared held-out algorithm families only. Primitive reuse and final-task exactness remain separate metrics so primitive-layer transfer does not get overstated as full executor closure",
            ),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_shared_primitive_transfer_evidence_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Shared primitive transfer build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarSharedPrimitiveTransferError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the committed shared primitive transfer evidence bundle.
#[must_use]
pub fn build_tassadar_shared_primitive_transfer_evidence_bundle(
) -> TassadarSharedPrimitiveTransferEvidenceBundle {
    TassadarSharedPrimitiveTransferEvidenceBundle::new(transfer_cases(), primitive_ablations())
}

/// Returns the canonical absolute path for the committed evidence bundle.
#[must_use]
pub fn tassadar_shared_primitive_transfer_evidence_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF)
}

/// Writes the committed shared primitive transfer evidence bundle.
pub fn write_tassadar_shared_primitive_transfer_evidence_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSharedPrimitiveTransferEvidenceBundle, TassadarSharedPrimitiveTransferError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSharedPrimitiveTransferError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_shared_primitive_transfer_evidence_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSharedPrimitiveTransferError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn transfer_cases() -> Vec<TassadarSharedPrimitiveTransferCase> {
    vec![
        transfer_case(
            "sort_merge_zero_shot",
            TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
            TassadarSharedPrimitiveTransferRegime::ZeroShot,
            &[
                "tassadar.primitive.compare_candidates.v1",
                "tassadar.primitive.select_candidate.v1",
                "tassadar.primitive.merge_state.v1",
            ],
            8_600,
            8_100,
            &[
                "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
            ],
            "Held-out sort/merge stays strong at the primitive layer under zero-shot reuse, but some composition loss remains in the merge stage.",
        ),
        transfer_case(
            "sort_merge_few_shot",
            TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
            TassadarSharedPrimitiveTransferRegime::FewShot,
            &[
                "tassadar.primitive.compare_candidates.v1",
                "tassadar.primitive.select_candidate.v1",
                "tassadar.primitive.merge_state.v1",
            ],
            9_100,
            8_800,
            &[
                "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
            ],
            "Few-shot adaptation largely closes the sort/merge composition gap while staying bounded to the published primitive vocabulary.",
        ),
        transfer_case(
            "clrs_shortest_path_zero_shot",
            TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
            TassadarSharedPrimitiveTransferRegime::ZeroShot,
            &[
                "tassadar.primitive.reachability_expand.v1",
                "tassadar.primitive.relax_state.v1",
                "tassadar.primitive.compare_candidates.v1",
                "tassadar.primitive.merge_state.v1",
            ],
            8_200,
            7_600,
            &[
                "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            ],
            "Shortest-path transfer is mostly present at the primitive layer, but relaxation and frontier merge still cost final-task exactness under zero-shot reuse.",
        ),
        transfer_case(
            "clrs_shortest_path_few_shot",
            TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
            TassadarSharedPrimitiveTransferRegime::FewShot,
            &[
                "tassadar.primitive.reachability_expand.v1",
                "tassadar.primitive.relax_state.v1",
                "tassadar.primitive.compare_candidates.v1",
                "tassadar.primitive.merge_state.v1",
            ],
            8_900,
            8_400,
            &[
                "fixtures/tassadar/reports/tassadar_subroutine_library_ablation_report.json",
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            ],
            "Few-shot shortest-path reuse improves both relax and merge composition while remaining below compiled exactness.",
        ),
        transfer_case(
            "clrs_wasm_shortest_path_zero_shot",
            TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
            TassadarSharedPrimitiveTransferRegime::ZeroShot,
            &[
                "tassadar.primitive.reachability_expand.v1",
                "tassadar.primitive.relax_state.v1",
                "tassadar.primitive.compare_candidates.v1",
            ],
            7_800,
            7_100,
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_learnability_gap_report.json",
            ],
            "The CLRS-to-Wasm bridge survives zero-shot primitive transfer better than full-task composition, so the bridge remains an honest held-out transfer target rather than a solved executor family.",
        ),
        transfer_case(
            "clrs_wasm_shortest_path_few_shot",
            TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
            TassadarSharedPrimitiveTransferRegime::FewShot,
            &[
                "tassadar.primitive.reachability_expand.v1",
                "tassadar.primitive.relax_state.v1",
                "tassadar.primitive.compare_candidates.v1",
            ],
            8_600,
            8_100,
            &[
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
                "fixtures/tassadar/reports/tassadar_learnability_gap_report.json",
            ],
            "Few-shot CLRS-to-Wasm reuse gains primitive stability, but end-to-end exactness still trails the compiled bridge.",
        ),
        transfer_case(
            "hungarian_matching_zero_shot",
            TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
            TassadarSharedPrimitiveTransferRegime::ZeroShot,
            &[
                "tassadar.primitive.relax_state.v1",
                "tassadar.primitive.compare_candidates.v1",
                "tassadar.primitive.select_candidate.v1",
                "tassadar.primitive.merge_state.v1",
            ],
            7_400,
            6_700,
            &[
                "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json",
                "fixtures/tassadar/reports/tassadar_learnability_gap_report.json",
            ],
            "Hungarian matching shows real primitive reuse, but the primitive layer itself is still weak enough that selection quality and merge fidelity both lag under zero-shot transfer.",
        ),
        transfer_case(
            "hungarian_matching_few_shot",
            TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
            TassadarSharedPrimitiveTransferRegime::FewShot,
            &[
                "tassadar.primitive.relax_state.v1",
                "tassadar.primitive.compare_candidates.v1",
                "tassadar.primitive.select_candidate.v1",
                "tassadar.primitive.merge_state.v1",
            ],
            8_300,
            7_600,
            &[
                "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json",
                "fixtures/tassadar/reports/tassadar_learnability_gap_report.json",
            ],
            "Few-shot Hungarian transfer improves the primitive layer materially, but full assignment composition still trails the compiled anchor.",
        ),
        transfer_case(
            "sudoku_search_zero_shot",
            TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
            TassadarSharedPrimitiveTransferRegime::ZeroShot,
            &[
                "tassadar.primitive.reachability_expand.v1",
                "tassadar.primitive.select_candidate.v1",
                "tassadar.primitive.bounded_backtrack.v1",
            ],
            7_000,
            5_600,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_learnability_gap_report.json",
            ],
            "Sudoku search exposes both weak primitive transfer and a large composition gap, so the lane still needs explicit refusal and research-only posture.",
        ),
        transfer_case(
            "sudoku_search_few_shot",
            TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
            TassadarSharedPrimitiveTransferRegime::FewShot,
            &[
                "tassadar.primitive.reachability_expand.v1",
                "tassadar.primitive.select_candidate.v1",
                "tassadar.primitive.bounded_backtrack.v1",
            ],
            8_100,
            6_900,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
                "fixtures/tassadar/reports/tassadar_learnability_gap_report.json",
            ],
            "Few-shot Sudoku search recovers the primitive layer but still leaves a large search-composition gap centered on bounded backtrack orchestration.",
        ),
        transfer_case(
            "verifier_search_kernel_zero_shot",
            TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            TassadarSharedPrimitiveTransferRegime::ZeroShot,
            &[
                "tassadar.primitive.reachability_expand.v1",
                "tassadar.primitive.select_candidate.v1",
                "tassadar.primitive.bounded_backtrack.v1",
            ],
            8_500,
            7_900,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            ],
            "The synthetic verifier-guided kernel keeps its primitive layer strong under zero-shot reuse, which makes it a useful contrast against the harder Sudoku composition story.",
        ),
        transfer_case(
            "verifier_search_kernel_few_shot",
            TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            TassadarSharedPrimitiveTransferRegime::FewShot,
            &[
                "tassadar.primitive.reachability_expand.v1",
                "tassadar.primitive.select_candidate.v1",
                "tassadar.primitive.bounded_backtrack.v1",
            ],
            9_200,
            8_700,
            &[
                "fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json",
            ],
            "Few-shot verifier-guided search approaches the published primitive ceiling while remaining a research substrate rather than a promoted solver claim.",
        ),
    ]
}

fn primitive_ablations() -> Vec<TassadarSharedPrimitiveAblationReceipt> {
    vec![
        TassadarSharedPrimitiveAblationReceipt {
            primitive_id: String::from("tassadar.primitive.reachability_expand.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::ReachabilityExpand,
            held_out_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
                TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            ],
            mean_zero_shot_drop_bps: 950,
            mean_few_shot_drop_bps: 700,
            detail: String::from(
                "frontier expansion is one of the cross-family primitives that still matters after few-shot adaptation",
            ),
        },
        TassadarSharedPrimitiveAblationReceipt {
            primitive_id: String::from("tassadar.primitive.relax_state.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::RelaxState,
            held_out_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsWasmShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
            ],
            mean_zero_shot_drop_bps: 1_100,
            mean_few_shot_drop_bps: 800,
            detail: String::from(
                "relaxation remains a foundational primitive for shortest-path style transfer and still materially affects matching-family transfer",
            ),
        },
        TassadarSharedPrimitiveAblationReceipt {
            primitive_id: String::from("tassadar.primitive.compare_candidates.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::CompareCandidates,
            held_out_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
            ],
            mean_zero_shot_drop_bps: 850,
            mean_few_shot_drop_bps: 600,
            detail: String::from(
                "candidate comparison transfers broadly, but its impact is smaller than relaxation or bounded backtrack on the current held-out set",
            ),
        },
        TassadarSharedPrimitiveAblationReceipt {
            primitive_id: String::from("tassadar.primitive.select_candidate.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::SelectCandidate,
            held_out_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
                TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
                TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            ],
            mean_zero_shot_drop_bps: 900,
            mean_few_shot_drop_bps: 650,
            detail: String::from(
                "selection is a broad cross-family primitive whose loss hurts both search and matching transfer regimes",
            ),
        },
        TassadarSharedPrimitiveAblationReceipt {
            primitive_id: String::from("tassadar.primitive.merge_state.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::MergeState,
            held_out_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SortMerge,
                TassadarSharedPrimitiveAlgorithmFamily::ClrsShortestPath,
                TassadarSharedPrimitiveAlgorithmFamily::HungarianMatching,
            ],
            mean_zero_shot_drop_bps: 700,
            mean_few_shot_drop_bps: 550,
            detail: String::from(
                "merge stays useful, but it is not the main bottleneck once the stronger relax/select/backtrack primitives are present",
            ),
        },
        TassadarSharedPrimitiveAblationReceipt {
            primitive_id: String::from("tassadar.primitive.bounded_backtrack.v1"),
            primitive_kind: TassadarSharedPrimitiveKind::BoundedBacktrack,
            held_out_algorithm_families: vec![
                TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch,
                TassadarSharedPrimitiveAlgorithmFamily::VerifierSearchKernel,
            ],
            mean_zero_shot_drop_bps: 1_400,
            mean_few_shot_drop_bps: 1_050,
            detail: String::from(
                "bounded backtrack is the most fragile but also the most valuable primitive on the current search-family transfer set",
            ),
        },
    ]
}

fn transfer_case(
    case_id: &str,
    held_out_algorithm_family: TassadarSharedPrimitiveAlgorithmFamily,
    regime: TassadarSharedPrimitiveTransferRegime,
    reused_primitive_ids: &[&str],
    primitive_reuse_bps: u32,
    final_task_exactness_bps: u32,
    evidence_refs: &[&str],
    detail: &str,
) -> TassadarSharedPrimitiveTransferCase {
    TassadarSharedPrimitiveTransferCase {
        case_id: String::from(case_id),
        held_out_algorithm_family,
        regime,
        reused_primitive_ids: reused_primitive_ids
            .iter()
            .map(|primitive_id| String::from(*primitive_id))
            .collect(),
        primitive_reuse_bps,
        final_task_exactness_bps,
        evidence_refs: evidence_refs
            .iter()
            .map(|reference| String::from(*reference))
            .collect(),
        detail: String::from(detail),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-train crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarSharedPrimitiveTransferError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| TassadarSharedPrimitiveTransferError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSharedPrimitiveTransferError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
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
        build_tassadar_shared_primitive_transfer_evidence_bundle, read_repo_json,
        tassadar_shared_primitive_transfer_evidence_bundle_path,
        write_tassadar_shared_primitive_transfer_evidence_bundle,
        TassadarSharedPrimitiveTransferEvidenceBundle, TassadarSharedPrimitiveTransferRegime,
    };
    use psionic_data::{
        TassadarSharedPrimitiveAlgorithmFamily,
        TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF,
    };

    #[test]
    fn shared_primitive_transfer_bundle_keeps_zero_shot_and_composition_gaps_separate() {
        let bundle = build_tassadar_shared_primitive_transfer_evidence_bundle();

        assert_eq!(bundle.transfer_cases.len(), 12);
        let sudoku_zero = bundle
            .transfer_cases
            .iter()
            .find(|case| {
                case.held_out_algorithm_family
                    == TassadarSharedPrimitiveAlgorithmFamily::SudokuSearch
                    && case.regime == TassadarSharedPrimitiveTransferRegime::ZeroShot
            })
            .expect("sudoku zero-shot case");
        assert!(sudoku_zero.primitive_reuse_bps > sudoku_zero.final_task_exactness_bps);
        assert!(bundle
            .primitive_ablations
            .iter()
            .any(|receipt| receipt.mean_zero_shot_drop_bps >= 1_000));
    }

    #[test]
    fn shared_primitive_transfer_bundle_matches_committed_truth() {
        let generated = build_tassadar_shared_primitive_transfer_evidence_bundle();
        let committed: TassadarSharedPrimitiveTransferEvidenceBundle =
            read_repo_json(TASSADAR_SHARED_PRIMITIVE_TRANSFER_EVIDENCE_BUNDLE_REF)
                .expect("committed bundle");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_shared_primitive_transfer_bundle_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("shared_primitive_transfer_evidence_bundle.json");
        let written = write_tassadar_shared_primitive_transfer_evidence_bundle(&output_path)
            .expect("write bundle");
        let persisted: TassadarSharedPrimitiveTransferEvidenceBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_shared_primitive_transfer_evidence_bundle_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("shared_primitive_transfer_evidence_bundle.json")
        );
    }
}
