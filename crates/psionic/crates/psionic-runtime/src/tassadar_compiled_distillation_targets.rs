use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u16 = 1;

pub const TASSADAR_COMPILED_DISTILLATION_TARGET_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/tassadar_compiled_distillation_targets_v1/compiled_distillation_target_bundle.json";

/// Runtime-visible workload family used by the distillation target bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRuntimeDistillationWorkloadFamily {
    KernelArithmetic,
    ClrsWasmShortestPath,
    HungarianMatching,
    SudokuSearch,
}

/// Runtime-visible invariance class emitted by the compiled/reference lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRuntimeDistillationInvarianceClass {
    OutputEquivalence,
    ProgressMonotonicity,
    StateDigestEquivalence,
    SelectionStability,
}

/// One compiled/reference authority case inside the distillation target bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationCaseTarget {
    pub case_id: String,
    pub workload_family: TassadarRuntimeDistillationWorkloadFamily,
    pub trace_abi_profile_id: String,
    pub final_output_targets: Vec<String>,
    pub partial_state_targets: Vec<String>,
    pub invariance_classes: Vec<TassadarRuntimeDistillationInvarianceClass>,
    pub compiled_authority_refs: Vec<String>,
    pub claim_boundary: String,
}

/// Runtime-owned target bundle for compiled-to-learned distillation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledDistillationTargetBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub cases: Vec<TassadarCompiledDistillationCaseTarget>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

impl TassadarCompiledDistillationTargetBundle {
    fn new(cases: Vec<TassadarCompiledDistillationCaseTarget>) -> Self {
        let mut bundle = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            bundle_id: String::from("tassadar.compiled_distillation.target_bundle.v1"),
            cases,
            claim_boundary: String::from(
                "this bundle emits compiled/reference final-output, partial-state, and invariance-class targets for bounded Tassadar workload families only. It is an authority surface for distillation targets, not a learned executor claim",
            ),
            summary: String::new(),
            bundle_digest: String::new(),
        };
        bundle.summary = format!(
            "Compiled distillation target bundle now freezes {} bounded authority cases across kernel, CLRS-to-Wasm, Hungarian, and Sudoku families.",
            bundle.cases.len(),
        );
        bundle.bundle_digest =
            stable_digest(b"psionic_tassadar_compiled_distillation_target_bundle|", &bundle);
        bundle
    }
}

/// Distillation target bundle build or persistence failure.
#[derive(Debug, Error)]
pub enum TassadarCompiledDistillationTargetBundleError {
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

/// Builds the committed compiled distillation target bundle.
#[must_use]
pub fn build_tassadar_compiled_distillation_target_bundle() -> TassadarCompiledDistillationTargetBundle
{
    TassadarCompiledDistillationTargetBundle::new(vec![
        TassadarCompiledDistillationCaseTarget {
            case_id: String::from("kernel_arithmetic_reference"),
            workload_family: TassadarRuntimeDistillationWorkloadFamily::KernelArithmetic,
            trace_abi_profile_id: String::from("tassadar.wasm.core_i32.v2"),
            final_output_targets: vec![
                String::from("register.r0=exact"),
                String::from("memory.delta.digest=stable"),
            ],
            partial_state_targets: vec![
                String::from("alu.window.step_digest"),
                String::from("branch_guard.stability"),
            ],
            invariance_classes: vec![
                TassadarRuntimeDistillationInvarianceClass::OutputEquivalence,
                TassadarRuntimeDistillationInvarianceClass::ProgressMonotonicity,
            ],
            compiled_authority_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_compile_pipeline_matrix_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"),
            ],
            claim_boundary: String::from(
                "kernel arithmetic remains a bounded compiled/reference source of distillation targets only",
            ),
        },
        TassadarCompiledDistillationCaseTarget {
            case_id: String::from("clrs_wasm_shortest_path_reference"),
            workload_family: TassadarRuntimeDistillationWorkloadFamily::ClrsWasmShortestPath,
            trace_abi_profile_id: String::from("tassadar.wasm.article_i32_compute.v1"),
            final_output_targets: vec![
                String::from("distance_table.digest"),
                String::from("predecessor_table.digest"),
            ],
            partial_state_targets: vec![
                String::from("frontier.digest"),
                String::from("relaxation_commit.sequence"),
            ],
            invariance_classes: vec![
                TassadarRuntimeDistillationInvarianceClass::OutputEquivalence,
                TassadarRuntimeDistillationInvarianceClass::StateDigestEquivalence,
                TassadarRuntimeDistillationInvarianceClass::ProgressMonotonicity,
            ],
            compiled_authority_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_clrs_wasm_bridge_report.json",
            )],
            claim_boundary: String::from(
                "the CLRS-to-Wasm bridge remains a bounded compiled/reference authority surface for distillation targets only",
            ),
        },
        TassadarCompiledDistillationCaseTarget {
            case_id: String::from("hungarian_matching_reference"),
            workload_family: TassadarRuntimeDistillationWorkloadFamily::HungarianMatching,
            trace_abi_profile_id: String::from("tassadar.wasm.hungarian_v0_matching.v1"),
            final_output_targets: vec![
                String::from("matching.assignment.digest"),
                String::from("cost.total=exact"),
            ],
            partial_state_targets: vec![
                String::from("row_selection.state"),
                String::from("column_commit.digest"),
            ],
            invariance_classes: vec![
                TassadarRuntimeDistillationInvarianceClass::OutputEquivalence,
                TassadarRuntimeDistillationInvarianceClass::SelectionStability,
                TassadarRuntimeDistillationInvarianceClass::StateDigestEquivalence,
            ],
            compiled_authority_refs: vec![String::from(
                "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json",
            )],
            claim_boundary: String::from(
                "Hungarian matching remains a bounded compiled/reference source of distillation targets over the current witnesses only",
            ),
        },
        TassadarCompiledDistillationCaseTarget {
            case_id: String::from("sudoku_search_reference"),
            workload_family: TassadarRuntimeDistillationWorkloadFamily::SudokuSearch,
            trace_abi_profile_id: String::from("tassadar.wasm.sudoku_v0_search.v1"),
            final_output_targets: vec![
                String::from("board.solution.digest"),
                String::from("contradiction_count=exact"),
            ],
            partial_state_targets: vec![
                String::from("candidate_frontier.digest"),
                String::from("backtrack_checkpoint.digest"),
            ],
            invariance_classes: vec![
                TassadarRuntimeDistillationInvarianceClass::OutputEquivalence,
                TassadarRuntimeDistillationInvarianceClass::ProgressMonotonicity,
                TassadarRuntimeDistillationInvarianceClass::SelectionStability,
            ],
            compiled_authority_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_verifier_guided_search_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_exactness_refusal_report.json"),
            ],
            claim_boundary: String::from(
                "Sudoku search distillation targets stay bounded to the published compiled/reference search lane and do not imply broad learned search closure",
            ),
        },
    ])
}

/// Returns the canonical absolute path for the committed target bundle.
#[must_use]
pub fn tassadar_compiled_distillation_target_bundle_path() -> PathBuf {
    repo_root().join(TASSADAR_COMPILED_DISTILLATION_TARGET_BUNDLE_REF)
}

/// Writes the committed compiled distillation target bundle.
pub fn write_tassadar_compiled_distillation_target_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarCompiledDistillationTargetBundle, TassadarCompiledDistillationTargetBundleError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarCompiledDistillationTargetBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_compiled_distillation_target_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarCompiledDistillationTargetBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarCompiledDistillationTargetBundleError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarCompiledDistillationTargetBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarCompiledDistillationTargetBundleError::Deserialize {
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
        build_tassadar_compiled_distillation_target_bundle, read_repo_json,
        tassadar_compiled_distillation_target_bundle_path,
        write_tassadar_compiled_distillation_target_bundle,
        TassadarCompiledDistillationTargetBundle, TassadarRuntimeDistillationWorkloadFamily,
        TASSADAR_COMPILED_DISTILLATION_TARGET_BUNDLE_REF,
    };

    #[test]
    fn compiled_distillation_target_bundle_is_machine_legible() {
        let bundle = build_tassadar_compiled_distillation_target_bundle();

        assert_eq!(bundle.cases.len(), 4);
        assert!(bundle.cases.iter().any(|case| {
            case.workload_family == TassadarRuntimeDistillationWorkloadFamily::SudokuSearch
                && case
                    .partial_state_targets
                    .contains(&String::from("backtrack_checkpoint.digest"))
        }));
        assert!(!bundle.bundle_digest.is_empty());
    }

    #[test]
    fn compiled_distillation_target_bundle_matches_committed_truth() {
        let generated = build_tassadar_compiled_distillation_target_bundle();
        let committed: TassadarCompiledDistillationTargetBundle =
            read_repo_json(TASSADAR_COMPILED_DISTILLATION_TARGET_BUNDLE_REF)
                .expect("committed bundle");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_compiled_distillation_target_bundle_persists_current_truth() {
        let output_path = std::env::temp_dir().join(format!(
            "compiled_distillation_target_bundle-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos()
        ));
        let written = write_tassadar_compiled_distillation_target_bundle(&output_path)
            .expect("write bundle");
        let persisted: TassadarCompiledDistillationTargetBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        let _ = std::fs::remove_file(&output_path);
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_compiled_distillation_target_bundle_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("compiled_distillation_target_bundle.json")
        );
    }
}
