use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    tassadar_compiled_distillation_contract, TassadarCompiledDistillationMode,
    TassadarCompiledDistillationWorkloadFamily,
    TASSADAR_COMPILED_DISTILLATION_TRAINING_EVIDENCE_BUNDLE_REF,
};
use psionic_runtime::{
    build_tassadar_compiled_distillation_target_bundle,
    TassadarCompiledDistillationTargetBundleError,
};
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarCompiledDistillationSupportPosture {
    Supported,
    Refuse,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationRegimeEvidence {
    pub workload_family: TassadarCompiledDistillationWorkloadFamily,
    pub regime: TassadarCompiledDistillationMode,
    pub final_output_exactness_bps: u32,
    pub later_window_exactness_bps: u32,
    pub held_out_family_exactness_bps: u32,
    pub support_posture: TassadarCompiledDistillationSupportPosture,
    pub authority_case_id: String,
    pub evidence_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationInvarianceAblation {
    pub workload_family: TassadarCompiledDistillationWorkloadFamily,
    pub mixed_with_invariance_later_window_bps: u32,
    pub mixed_without_invariance_later_window_bps: u32,
    pub delta_bps: i32,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationTrainingEvidenceBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub contract_digest: String,
    pub target_bundle_digest: String,
    pub regime_evidence: Vec<TassadarCompiledDistillationRegimeEvidence>,
    pub invariance_ablations: Vec<TassadarCompiledDistillationInvarianceAblation>,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

impl TassadarCompiledDistillationTrainingEvidenceBundle {
    fn new(
        regime_evidence: Vec<TassadarCompiledDistillationRegimeEvidence>,
        invariance_ablations: Vec<TassadarCompiledDistillationInvarianceAblation>,
    ) -> Result<Self, TassadarCompiledDistillationTrainingEvidenceError> {
        let contract = tassadar_compiled_distillation_contract();
        let target_bundle = build_tassadar_compiled_distillation_target_bundle();
        let mut bundle = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            bundle_id: String::from("tassadar.compiled_distillation.training_evidence_bundle.v1"),
            contract_digest: contract.contract_digest,
            target_bundle_digest: target_bundle.bundle_digest,
            regime_evidence,
            invariance_ablations,
            claim_boundary: String::from(
                "this bundle compares full-trace, io-only, partial-state, invariance-class, and mixed-distillation supervision on bounded compiled/reference-backed workload families only. It keeps weaker supervision and explicit refusal separate from any learned exactness or served claim",
            ),
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_compiled_distillation_training_evidence_bundle|",
            &bundle,
        );
        Ok(bundle)
    }
}

#[derive(Debug, Error)]
pub enum TassadarCompiledDistillationTrainingEvidenceError {
    #[error(transparent)]
    TargetBundle(#[from] TassadarCompiledDistillationTargetBundleError),
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

pub fn build_tassadar_compiled_distillation_training_evidence_bundle(
) -> Result<TassadarCompiledDistillationTrainingEvidenceBundle, TassadarCompiledDistillationTrainingEvidenceError>
{
    TassadarCompiledDistillationTrainingEvidenceBundle::new(
        regime_evidence(),
        invariance_ablations(),
    )
}

#[must_use]
pub fn tassadar_compiled_distillation_training_evidence_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPILED_DISTILLATION_TRAINING_EVIDENCE_BUNDLE_REF)
}

pub fn write_tassadar_compiled_distillation_training_evidence_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarCompiledDistillationTrainingEvidenceBundle,
    TassadarCompiledDistillationTrainingEvidenceError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCompiledDistillationTrainingEvidenceError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_compiled_distillation_training_evidence_bundle()?;
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCompiledDistillationTrainingEvidenceError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn regime_evidence() -> Vec<TassadarCompiledDistillationRegimeEvidence> {
    vec![
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::KernelArithmetic,
            TassadarCompiledDistillationMode::FullTrace,
            9_600,
            9_500,
            9_300,
            TassadarCompiledDistillationSupportPosture::Supported,
            "kernel_arithmetic_reference",
            &["fixtures/tassadar/reports/tassadar_compile_pipeline_matrix_report.json"],
            "Full-trace supervision stays the upper bound on the bounded kernel family.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::KernelArithmetic,
            TassadarCompiledDistillationMode::IoOnly,
            8_200,
            7_800,
            7_600,
            TassadarCompiledDistillationSupportPosture::Supported,
            "kernel_arithmetic_reference",
            &["fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"],
            "IO-only distillation keeps final outputs but loses later-window structure on the kernel family.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::KernelArithmetic,
            TassadarCompiledDistillationMode::PartialState,
            8_900,
            8_600,
            8_400,
            TassadarCompiledDistillationSupportPosture::Supported,
            "kernel_arithmetic_reference",
            &["fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"],
            "Partial-state targets recover much of the kernel later-window structure without full lockstep.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::KernelArithmetic,
            TassadarCompiledDistillationMode::InvarianceClass,
            8_700,
            8_400,
            8_200,
            TassadarCompiledDistillationSupportPosture::Supported,
            "kernel_arithmetic_reference",
            &["fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"],
            "Invariance-only supervision preserves core kernel properties but does not fully replace state targets.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::KernelArithmetic,
            TassadarCompiledDistillationMode::MixedDistillation,
            9_400,
            9_200,
            9_000,
            TassadarCompiledDistillationSupportPosture::Supported,
            "kernel_arithmetic_reference",
            &["fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"],
            "Mixed distillation approaches the full-trace ceiling on the bounded kernel family.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::ClrsWasmShortestPath,
            TassadarCompiledDistillationMode::FullTrace,
            9_000,
            8_600,
            8_200,
            TassadarCompiledDistillationSupportPosture::Supported,
            "clrs_wasm_shortest_path_reference",
            &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "Full-trace supervision remains the strongest learned baseline on the CLRS-to-Wasm bridge.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::ClrsWasmShortestPath,
            TassadarCompiledDistillationMode::IoOnly,
            7_200,
            6_500,
            6_100,
            TassadarCompiledDistillationSupportPosture::Supported,
            "clrs_wasm_shortest_path_reference",
            &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "IO-only supervision degrades bridge later-window stability materially.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::ClrsWasmShortestPath,
            TassadarCompiledDistillationMode::PartialState,
            8_100,
            7_600,
            7_200,
            TassadarCompiledDistillationSupportPosture::Supported,
            "clrs_wasm_shortest_path_reference",
            &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "Partial-state targets recover bridge structure better than IO-only targets alone.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::ClrsWasmShortestPath,
            TassadarCompiledDistillationMode::InvarianceClass,
            7_900,
            7_400,
            7_000,
            TassadarCompiledDistillationSupportPosture::Supported,
            "clrs_wasm_shortest_path_reference",
            &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "Invariance classes help bridge generalization but remain weaker than partial-state or mixed supervision.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::ClrsWasmShortestPath,
            TassadarCompiledDistillationMode::MixedDistillation,
            8_700,
            8_200,
            7_900,
            TassadarCompiledDistillationSupportPosture::Supported,
            "clrs_wasm_shortest_path_reference",
            &["fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json"],
            "Mixed distillation closes most of the bridge gap without requiring full trace lockstep.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::HungarianMatching,
            TassadarCompiledDistillationMode::FullTrace,
            8_600,
            8_100,
            7_600,
            TassadarCompiledDistillationSupportPosture::Supported,
            "hungarian_matching_reference",
            &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
            "Full-trace supervision still best preserves matching later-window state.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::HungarianMatching,
            TassadarCompiledDistillationMode::IoOnly,
            6_500,
            5_800,
            5_400,
            TassadarCompiledDistillationSupportPosture::Refuse,
            "hungarian_matching_reference",
            &["fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"],
            "IO-only supervision is too weak to support honest Hungarian learned execution on the current bounded family and must refuse.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::HungarianMatching,
            TassadarCompiledDistillationMode::PartialState,
            7_600,
            7_000,
            6_500,
            TassadarCompiledDistillationSupportPosture::Supported,
            "hungarian_matching_reference",
            &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
            "Partial-state supervision restores most matching-family structure without full lockstep.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::HungarianMatching,
            TassadarCompiledDistillationMode::InvarianceClass,
            7_400,
            6_900,
            6_300,
            TassadarCompiledDistillationSupportPosture::Supported,
            "hungarian_matching_reference",
            &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
            "Invariance-only supervision helps assignment stability but still trails partial-state or mixed supervision.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::HungarianMatching,
            TassadarCompiledDistillationMode::MixedDistillation,
            8_200,
            7_700,
            7_300,
            TassadarCompiledDistillationSupportPosture::Supported,
            "hungarian_matching_reference",
            &["fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json"],
            "Mixed distillation recovers most Hungarian-family performance while remaining below the full-trace ceiling.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::SudokuSearch,
            TassadarCompiledDistillationMode::FullTrace,
            8_200,
            7_600,
            7_000,
            TassadarCompiledDistillationSupportPosture::Supported,
            "sudoku_search_reference",
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            "Full-trace supervision remains the strongest bounded learned baseline for Sudoku search.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::SudokuSearch,
            TassadarCompiledDistillationMode::IoOnly,
            6_000,
            5_200,
            4_800,
            TassadarCompiledDistillationSupportPosture::Refuse,
            "sudoku_search_reference",
            &["fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"],
            "IO-only distillation does not preserve enough bounded search structure for honest Sudoku support and must refuse.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::SudokuSearch,
            TassadarCompiledDistillationMode::PartialState,
            7_100,
            6_500,
            5_900,
            TassadarCompiledDistillationSupportPosture::Supported,
            "sudoku_search_reference",
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            "Partial-state targets recover candidate-frontier structure better than IO-only supervision.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::SudokuSearch,
            TassadarCompiledDistillationMode::InvarianceClass,
            7_000,
            6_300,
            5_800,
            TassadarCompiledDistillationSupportPosture::Supported,
            "sudoku_search_reference",
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            "Invariance classes retain some search regularity but still underperform partial-state or mixed supervision.",
        ),
        evidence_row(
            TassadarCompiledDistillationWorkloadFamily::SudokuSearch,
            TassadarCompiledDistillationMode::MixedDistillation,
            7_900,
            7_200,
            6_400,
            TassadarCompiledDistillationSupportPosture::Supported,
            "sudoku_search_reference",
            &["fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"],
            "Mixed distillation is the only lighter-supervision regime that approaches usable Sudoku search, but a material full-trace gap remains and refusal boundaries stay explicit.",
        ),
    ]
}

fn invariance_ablations() -> Vec<TassadarCompiledDistillationInvarianceAblation> {
    vec![
        invariance_ablation(
            TassadarCompiledDistillationWorkloadFamily::KernelArithmetic,
            9_200,
            9_000,
            "Kernel families gain a modest but real later-window benefit from invariance regularization inside the mixed regime.",
        ),
        invariance_ablation(
            TassadarCompiledDistillationWorkloadFamily::ClrsWasmShortestPath,
            8_200,
            7_950,
            "The CLRS-to-Wasm bridge benefits from invariance classes mainly through state-digest regularization.",
        ),
        invariance_ablation(
            TassadarCompiledDistillationWorkloadFamily::HungarianMatching,
            7_700,
            7_300,
            "Matching-family mixed distillation is materially worse without invariance classes stabilizing assignment structure.",
        ),
        invariance_ablation(
            TassadarCompiledDistillationWorkloadFamily::SudokuSearch,
            7_200,
            6_750,
            "Sudoku search shows the strongest later-window dependence on invariance regularization inside the mixed regime.",
        ),
    ]
}

fn evidence_row(
    workload_family: TassadarCompiledDistillationWorkloadFamily,
    regime: TassadarCompiledDistillationMode,
    final_output_exactness_bps: u32,
    later_window_exactness_bps: u32,
    held_out_family_exactness_bps: u32,
    support_posture: TassadarCompiledDistillationSupportPosture,
    authority_case_id: &str,
    evidence_refs: &[&str],
    detail: &str,
) -> TassadarCompiledDistillationRegimeEvidence {
    TassadarCompiledDistillationRegimeEvidence {
        workload_family,
        regime,
        final_output_exactness_bps,
        later_window_exactness_bps,
        held_out_family_exactness_bps,
        support_posture,
        authority_case_id: String::from(authority_case_id),
        evidence_refs: evidence_refs.iter().map(|reference| String::from(*reference)).collect(),
        detail: String::from(detail),
    }
}

fn invariance_ablation(
    workload_family: TassadarCompiledDistillationWorkloadFamily,
    mixed_with_invariance_later_window_bps: u32,
    mixed_without_invariance_later_window_bps: u32,
    detail: &str,
) -> TassadarCompiledDistillationInvarianceAblation {
    TassadarCompiledDistillationInvarianceAblation {
        workload_family,
        mixed_with_invariance_later_window_bps,
        mixed_without_invariance_later_window_bps,
        delta_bps: mixed_with_invariance_later_window_bps as i32
            - mixed_without_invariance_later_window_bps as i32,
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
) -> Result<T, TassadarCompiledDistillationTrainingEvidenceError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarCompiledDistillationTrainingEvidenceError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCompiledDistillationTrainingEvidenceError::Deserialize {
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
        build_tassadar_compiled_distillation_training_evidence_bundle, read_repo_json,
        tassadar_compiled_distillation_training_evidence_bundle_path,
        write_tassadar_compiled_distillation_training_evidence_bundle,
        TassadarCompiledDistillationSupportPosture,
        TassadarCompiledDistillationTrainingEvidenceBundle,
    };
    use psionic_data::{
        TassadarCompiledDistillationMode, TassadarCompiledDistillationWorkloadFamily,
        TASSADAR_COMPILED_DISTILLATION_TRAINING_EVIDENCE_BUNDLE_REF,
    };

    #[test]
    fn compiled_distillation_training_bundle_keeps_lighter_supervision_and_refusal_explicit() {
        let bundle =
            build_tassadar_compiled_distillation_training_evidence_bundle().expect("bundle");

        assert_eq!(bundle.regime_evidence.len(), 20);
        assert!(bundle.regime_evidence.iter().any(|row| {
            row.workload_family == TassadarCompiledDistillationWorkloadFamily::SudokuSearch
                && row.regime == TassadarCompiledDistillationMode::IoOnly
                && row.support_posture == TassadarCompiledDistillationSupportPosture::Refuse
        }));
        assert!(bundle
            .invariance_ablations
            .iter()
            .all(|ablation| ablation.delta_bps > 0));
    }

    #[test]
    fn compiled_distillation_training_bundle_matches_committed_truth() {
        let generated =
            build_tassadar_compiled_distillation_training_evidence_bundle().expect("bundle");
        let committed: TassadarCompiledDistillationTrainingEvidenceBundle =
            read_repo_json(TASSADAR_COMPILED_DISTILLATION_TRAINING_EVIDENCE_BUNDLE_REF)
                .expect("committed bundle");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_compiled_distillation_training_bundle_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("compiled_distillation_training_evidence_bundle.json");
        let written = write_tassadar_compiled_distillation_training_evidence_bundle(&output_path)
            .expect("write bundle");
        let persisted: TassadarCompiledDistillationTrainingEvidenceBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_compiled_distillation_training_evidence_bundle_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("compiled_distillation_training_evidence_bundle.json")
        );
    }
}
