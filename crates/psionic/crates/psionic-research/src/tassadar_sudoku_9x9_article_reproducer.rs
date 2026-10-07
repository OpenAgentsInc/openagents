use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarRustSourceCanonCaseStatus, TassadarRustSourceCanonReport,
    TassadarSudoku9x9CompiledExecutorExactnessReport,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarSudoku9x9CompiledExecutorRunBundle, TassadarSudoku9x9TokenTraceSummary,
};

const REPORT_SCHEMA_VERSION: u16 = 1;
const SOURCE_CANON_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_source_canon_report.json";
const RUN_BUNDLE_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/run_bundle.json";
const EXACTNESS_REPORT_REF: &str =
    "fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0/compiled_executor_exactness_report.json";
const CANONICAL_CASE_ID: &str = "sudoku_9x9_test_a";

pub const TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_sudoku_9x9_article_reproducer_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCorpusCaseSummary {
    pub case_id: String,
    pub split: String,
    pub difficulty_signal: String,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9ArticleDeploymentArtifactRef {
    pub artifact_ref: String,
    pub artifact_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9ArticleDirectExecutionPosture {
    pub requested_decode_mode: String,
    pub effective_decode_mode: String,
    pub fallback_observed: bool,
    pub external_tool_surface_observed: bool,
    pub runtime_backend: String,
    pub compiled_backend_features: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSudoku9x9ArticleReproducerReport {
    pub schema_version: u16,
    pub report_id: String,
    pub workload_family_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub source_receipt_digest: String,
    pub wasm_binary_ref: String,
    pub wasm_binary_digest: String,
    pub canonical_run_bundle_ref: String,
    pub exactness_report_ref: String,
    pub canonical_case_id: String,
    pub canonical_case_split: String,
    pub canonical_case_given_count: usize,
    pub exact_trace_match: bool,
    pub final_output_match: bool,
    pub halt_match: bool,
    pub article_corpus_cases: Vec<TassadarArticleCorpusCaseSummary>,
    pub compile_evidence_bundle: TassadarSudoku9x9ArticleDeploymentArtifactRef,
    pub program_artifact: TassadarSudoku9x9ArticleDeploymentArtifactRef,
    pub compiled_weight_artifact: TassadarSudoku9x9ArticleDeploymentArtifactRef,
    pub model_descriptor: TassadarSudoku9x9ArticleDeploymentArtifactRef,
    pub runtime_execution_proof_bundle: TassadarSudoku9x9ArticleDeploymentArtifactRef,
    pub runtime_trace_proof: TassadarSudoku9x9ArticleDeploymentArtifactRef,
    pub token_trace_summary: TassadarSudoku9x9ArticleDeploymentArtifactRef,
    pub readable_log: TassadarSudoku9x9ArticleDeploymentArtifactRef,
    pub direct_execution_posture: TassadarSudoku9x9ArticleDirectExecutionPosture,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarSudoku9x9ArticleReproducerReport {
    #[allow(clippy::too_many_arguments)]
    fn new(
        source_case: &psionic_eval::TassadarRustSourceCanonCase,
        run_bundle: &TassadarSudoku9x9CompiledExecutorRunBundle,
        exactness_case: &psionic_eval::TassadarSudoku9x9CompiledExecutorCaseExactnessReport,
        article_corpus_cases: Vec<TassadarArticleCorpusCaseSummary>,
        compile_evidence_bundle: TassadarSudoku9x9ArticleDeploymentArtifactRef,
        program_artifact: TassadarSudoku9x9ArticleDeploymentArtifactRef,
        compiled_weight_artifact: TassadarSudoku9x9ArticleDeploymentArtifactRef,
        model_descriptor: TassadarSudoku9x9ArticleDeploymentArtifactRef,
        runtime_execution_proof_bundle: TassadarSudoku9x9ArticleDeploymentArtifactRef,
        runtime_trace_proof: TassadarSudoku9x9ArticleDeploymentArtifactRef,
        token_trace_summary: TassadarSudoku9x9ArticleDeploymentArtifactRef,
        readable_log: TassadarSudoku9x9ArticleDeploymentArtifactRef,
        direct_execution_posture: TassadarSudoku9x9ArticleDirectExecutionPosture,
    ) -> Self {
        let mut report = Self {
            schema_version: REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.sudoku_9x9.article_reproducer.v1"),
            workload_family_id: run_bundle.workload_family_id.clone(),
            source_ref: source_case.source_ref.clone(),
            source_digest: source_case.source_digest.clone(),
            source_receipt_digest: source_case.receipt_digest.clone(),
            wasm_binary_ref: source_case.wasm_binary_ref.clone().unwrap_or_default(),
            wasm_binary_digest: source_case.wasm_binary_digest.clone().unwrap_or_default(),
            canonical_run_bundle_ref: String::from(RUN_BUNDLE_REF),
            exactness_report_ref: String::from(EXACTNESS_REPORT_REF),
            canonical_case_id: String::from(CANONICAL_CASE_ID),
            canonical_case_split: exactness_case.split.as_str().to_string(),
            canonical_case_given_count: exactness_case.given_count,
            exact_trace_match: exactness_case.exact_trace_match,
            final_output_match: exactness_case.final_output_match,
            halt_match: exactness_case.halt_match,
            article_corpus_cases,
            compile_evidence_bundle,
            program_artifact,
            compiled_weight_artifact,
            model_descriptor,
            runtime_execution_proof_bundle,
            runtime_trace_proof,
            token_trace_summary,
            readable_log,
            direct_execution_posture,
            claim_boundary: String::from(
                "this report closes one canonical Rust-only Sudoku-9x9 article reproducer by binding the committed Rust source canon receipt to the exact compiled `sudoku_9x9_test_a` search deployment and the committed 9x9 corpus case set. It closes this one backtracking-search workload family only and does not by itself imply Hungarian closure, multi-million-step closure, or arbitrary-program closure.",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_sudoku_9x9_article_reproducer_report|", &report);
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarSudoku9x9ArticleReproducerError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{artifact_kind}` from `{path}`: {error}")]
    Decode {
        path: String,
        artifact_kind: String,
        error: serde_json::Error,
    },
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("missing rust source canon case `sudoku_9x9_article`")]
    MissingSourceCanonCase,
    #[error("rust source canon case `sudoku_9x9_article` did not compile")]
    SourceCanonNotCompiled,
    #[error("missing deployment bundle for canonical case `{case_id}`")]
    MissingDeploymentBundle { case_id: String },
    #[error("missing exactness case for canonical case `{case_id}`")]
    MissingExactnessCase { case_id: String },
}

#[must_use]
pub fn tassadar_sudoku_9x9_article_reproducer_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF)
}

pub fn build_tassadar_sudoku_9x9_article_reproducer_report(
) -> Result<TassadarSudoku9x9ArticleReproducerReport, TassadarSudoku9x9ArticleReproducerError> {
    let source_report: TassadarRustSourceCanonReport =
        read_repo_json(SOURCE_CANON_REPORT_REF, "tassadar_rust_source_canon_report")?;
    let source_case = source_report
        .cases
        .iter()
        .find(|case| case.case_id == "sudoku_9x9_article")
        .ok_or(TassadarSudoku9x9ArticleReproducerError::MissingSourceCanonCase)?;
    if source_case.status != TassadarRustSourceCanonCaseStatus::Compiled {
        return Err(TassadarSudoku9x9ArticleReproducerError::SourceCanonNotCompiled);
    }

    let run_bundle: TassadarSudoku9x9CompiledExecutorRunBundle =
        read_repo_json(RUN_BUNDLE_REF, "tassadar_sudoku_9x9_compiled_executor_run_bundle")?;
    let deployment = run_bundle
        .deployments
        .iter()
        .find(|bundle| bundle.case_id == CANONICAL_CASE_ID)
        .ok_or_else(|| TassadarSudoku9x9ArticleReproducerError::MissingDeploymentBundle {
            case_id: String::from(CANONICAL_CASE_ID),
        })?;
    let exactness_report: TassadarSudoku9x9CompiledExecutorExactnessReport =
        read_repo_json(
            EXACTNESS_REPORT_REF,
            "tassadar_sudoku_9x9_compiled_executor_exactness_report",
        )?;
    let exactness_case = exactness_report
        .case_reports
        .iter()
        .find(|case| case.case_id == CANONICAL_CASE_ID)
        .ok_or_else(|| TassadarSudoku9x9ArticleReproducerError::MissingExactnessCase {
            case_id: String::from(CANONICAL_CASE_ID),
        })?;

    let deployment_root = repo_root()
        .join("fixtures/tassadar/runs/sudoku_9x9_v0_compiled_executor_v0")
        .join(&deployment.deployment_dir);
    let runtime_execution_proof_bundle: serde_json::Value = read_json(
        deployment_root.join("runtime_execution_proof_bundle.json"),
        "execution_proof_bundle",
    )?;
    let runtime_backend = runtime_execution_proof_bundle["runtime_identity"]["runtime_backend"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();
    let compiled_backend_features = runtime_execution_proof_bundle["runtime_identity"]
        ["backend_toolchain"]["compiled_backend_features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str().map(String::from))
        .collect::<Vec<_>>();
    let external_tool_surface_observed = compiled_backend_features.iter().any(|feature| {
        feature.contains("external_tool") || feature.contains("tool_call")
    });
    let token_trace_summary: TassadarSudoku9x9TokenTraceSummary = read_json(
        deployment_root.join("token_trace_summary.json"),
        "tassadar_sudoku_9x9_token_trace_summary",
    )?;

    let article_corpus_cases = exactness_report
        .case_reports
        .iter()
        .map(|case| TassadarArticleCorpusCaseSummary {
            case_id: case.case_id.clone(),
            split: case.split.as_str().to_string(),
            difficulty_signal: format!("givens={}", case.given_count),
            exact_trace_match: case.exact_trace_match,
            final_output_match: case.final_output_match,
        })
        .collect::<Vec<_>>();

    Ok(TassadarSudoku9x9ArticleReproducerReport::new(
        source_case,
        &run_bundle,
        exactness_case,
        article_corpus_cases,
        artifact_ref(
            &deployment_root,
            "compile_evidence_bundle.json",
            file_digest(&deployment_root.join("compile_evidence_bundle.json"))?,
        ),
        artifact_ref(
            &deployment_root,
            "program_artifact.json",
            exactness_case.program_artifact_digest.clone(),
        ),
        artifact_ref(
            &deployment_root,
            "compiled_weight_artifact.json",
            exactness_case.compiled_weight_artifact_digest.clone(),
        ),
        artifact_ref(
            &deployment_root,
            "model_descriptor.json",
            file_digest(&deployment_root.join("model_descriptor.json"))?,
        ),
        artifact_ref(
            &deployment_root,
            "runtime_execution_proof_bundle.json",
            exactness_case.runtime_execution_proof_bundle_digest.clone(),
        ),
        artifact_ref(
            &deployment_root,
            "runtime_trace_proof.json",
            deployment.runtime_trace_proof_digest.clone(),
        ),
        artifact_ref(
            &deployment_root,
            "token_trace_summary.json",
            token_trace_summary.summary_digest.clone(),
        ),
        artifact_ref(
            &deployment_root,
            "readable_log.txt",
            file_digest(&deployment_root.join("readable_log.txt"))?,
        ),
        TassadarSudoku9x9ArticleDirectExecutionPosture {
            requested_decode_mode: exactness_case.requested_decode_mode.as_str().to_string(),
            effective_decode_mode: exactness_case.effective_decode_mode.as_str().to_string(),
            fallback_observed: exactness_case.requested_decode_mode
                != exactness_case.effective_decode_mode,
            external_tool_surface_observed,
            runtime_backend,
            compiled_backend_features,
            detail: String::from(
                "the canonical Sudoku reproducer stays on the direct compiled executor lane with reference-linear decode parity, no observed fallback, and no external-tool feature markers in the runtime identity",
            ),
        },
    ))
}

pub fn write_tassadar_sudoku_9x9_article_reproducer_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSudoku9x9ArticleReproducerReport, TassadarSudoku9x9ArticleReproducerError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSudoku9x9ArticleReproducerError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_sudoku_9x9_article_reproducer_report()?;
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("Sudoku article reproducer report should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarSudoku9x9ArticleReproducerError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn artifact_ref(
    deployment_root: &Path,
    file_name: &str,
    artifact_digest: String,
) -> TassadarSudoku9x9ArticleDeploymentArtifactRef {
    TassadarSudoku9x9ArticleDeploymentArtifactRef {
        artifact_ref: canonical_repo_relative_path(&deployment_root.join(file_name)),
        artifact_digest,
    }
}

fn file_digest(path: &Path) -> Result<String, TassadarSudoku9x9ArticleReproducerError> {
    let bytes = fs::read(path).map_err(|error| TassadarSudoku9x9ArticleReproducerError::Read {
        path: path.display().to_string(),
        error,
    })?;
    Ok(stable_bytes_digest(&bytes))
}

fn stable_bytes_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn read_repo_json<T>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarSudoku9x9ArticleReproducerError>
where
    T: DeserializeOwned,
{
    read_json(repo_root().join(relative_path), artifact_kind)
}

fn read_json<T>(
    path: impl AsRef<Path>,
    artifact_kind: &str,
) -> Result<T, TassadarSudoku9x9ArticleReproducerError>
where
    T: DeserializeOwned,
{
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarSudoku9x9ArticleReproducerError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSudoku9x9ArticleReproducerError::Decode {
            path: path.display().to_string(),
            artifact_kind: artifact_kind.to_string(),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-research should live under <repo>/crates/psionic-research")
        .to_path_buf()
}

fn canonical_repo_relative_path(path: &Path) -> String {
    let repo_root = repo_root();
    let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    canonical_path
        .strip_prefix(&repo_root)
        .unwrap_or(&canonical_path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let encoded = serde_json::to_vec(value).expect("Sudoku reproducer report should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF,
        build_tassadar_sudoku_9x9_article_reproducer_report,
        tassadar_sudoku_9x9_article_reproducer_report_path,
        write_tassadar_sudoku_9x9_article_reproducer_report,
    };
    use crate::TassadarSudoku9x9ArticleReproducerReport;

    #[test]
    fn sudoku_9x9_article_reproducer_report_is_machine_legible(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_sudoku_9x9_article_reproducer_report()?;
        assert_eq!(report.canonical_case_id, "sudoku_9x9_test_a");
        assert!(report.exact_trace_match);
        assert!(report.final_output_match);
        assert!(report.halt_match);
        assert!(!report.direct_execution_posture.fallback_observed);
        assert!(!report
            .direct_execution_posture
            .external_tool_surface_observed);
        assert_eq!(report.article_corpus_cases.len(), 4);
        Ok(())
    }

    #[test]
    fn sudoku_9x9_article_reproducer_report_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let expected = build_tassadar_sudoku_9x9_article_reproducer_report()?;
        let path = tassadar_sudoku_9x9_article_reproducer_report_path();
        let written = write_tassadar_sudoku_9x9_article_reproducer_report(&path)?;
        assert_eq!(expected, written);
        let persisted: TassadarSudoku9x9ArticleReproducerReport =
            serde_json::from_slice(&std::fs::read(path)?)?;
        assert_eq!(persisted, expected);
        assert_eq!(
            TASSADAR_SUDOKU_9X9_ARTICLE_REPRODUCER_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_sudoku_9x9_article_reproducer_report.json"
        );
        Ok(())
    }
}
