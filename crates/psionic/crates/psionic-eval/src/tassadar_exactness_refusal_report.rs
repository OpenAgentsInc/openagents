use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    TASSADAR_ARTICLE_CLASS_BENCHMARK_REF, TassadarCpuReferenceRunner,
    TassadarExactnessRefusalReport, TassadarExecutorDecodeMode, TassadarValidationCase,
    diagnose_tassadar_executor_request, execute_tassadar_executor_request,
    tassadar_article_class_corpus,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_EXACTNESS_REFUSAL_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_EXACTNESS_REFUSAL_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_exactness_refusal_report.json";

/// One runtime evidence row inside the exactness/refusal artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactnessRefusalArtifactCase {
    /// Stable case identifier for the evidence row.
    pub case_id: String,
    /// Human-readable case summary.
    pub summary: String,
    /// Shared runtime exactness/refusal report.
    pub runtime_report: TassadarExactnessRefusalReport,
}

/// Committed machine-readable exactness/refusal artifact for Tassadar.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarExactnessRefusalArtifactReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Benchmark contract that anchored the runtime-backed cases.
    pub benchmark_ref: String,
    /// Exact, fallback, and refusal evidence rows in stable order.
    pub cases: Vec<TassadarExactnessRefusalArtifactCase>,
    /// Explicit claim boundary for the artifact.
    pub claim_boundary: String,
    /// Stable digest over the visible report body.
    pub report_digest: String,
}

impl TassadarExactnessRefusalArtifactReport {
    fn new(cases: Vec<TassadarExactnessRefusalArtifactCase>) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_EXACTNESS_REFUSAL_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.exactness_refusal_report.v1"),
            benchmark_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REF),
            cases,
            claim_boundary: String::from(
                "this artifact records one exact direct case, one exact fallback case, and one explicit refusal case for the current article-class runtime contract; it is a benchmark-bound evidence surface and does not widen served closure beyond the cited runtime selection and refusal facts",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(
            b"psionic_tassadar_exactness_refusal_artifact_report|",
            &report,
        );
        report
    }
}

/// Exactness/refusal artifact errors.
#[derive(Debug, Error)]
pub enum TassadarExactnessRefusalArtifactError {
    /// One canonical article case was missing.
    #[error("missing canonical Tassadar article case `{case_id}`")]
    MissingCase {
        /// Expected case id.
        case_id: String,
    },
    /// Building the CPU-reference execution failed.
    #[error("failed to run Tassadar CPU reference for `{case_id}`: {error}")]
    ReferenceExecution {
        /// Case identifier.
        case_id: String,
        /// Runtime error.
        error: psionic_runtime::TassadarExecutionRefusal,
    },
    /// Building the executor execution report failed.
    #[error("failed to execute Tassadar decode `{decode_mode}` for `{case_id}`: {detail}")]
    Execution {
        /// Case identifier.
        case_id: String,
        /// Requested decode mode.
        decode_mode: String,
        /// Runtime detail.
        detail: String,
    },
    /// Failed to create the output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the artifact.
    #[error("failed to write exactness/refusal artifact `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to read a committed artifact.
    #[error("failed to read committed exactness/refusal artifact `{path}`: {error}")]
    Read {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to decode a committed artifact.
    #[error("failed to decode committed exactness/refusal artifact `{path}`: {error}")]
    Decode {
        /// File path.
        path: String,
        /// JSON error.
        error: serde_json::Error,
    },
}

pub fn build_tassadar_exactness_refusal_artifact_report()
-> Result<TassadarExactnessRefusalArtifactReport, TassadarExactnessRefusalArtifactError> {
    let corpus = tassadar_article_class_corpus();
    let exact_case = find_case(&corpus, "micro_wasm_kernel")?;
    let fallback_case = find_case(&corpus, "long_loop_kernel")?;

    let exact_reference = run_reference(exact_case)?;
    let exact_execution = execute_tassadar_executor_request(
        &exact_case.program,
        TassadarExecutorDecodeMode::ReferenceLinear,
        psionic_runtime::tassadar_trace_abi_for_profile_id(exact_case.program.profile_id.as_str())
            .expect("article trace ABI should exist")
            .schema_version,
        None,
    )
    .map_err(
        |diagnostic| TassadarExactnessRefusalArtifactError::Execution {
            case_id: exact_case.case_id.clone(),
            decode_mode: String::from(TassadarExecutorDecodeMode::ReferenceLinear.as_str()),
            detail: diagnostic.detail,
        },
    )?;
    let exact_runtime_report = TassadarExactnessRefusalReport::from_execution_report(
        format!("{}:reference_linear", exact_case.case_id),
        &exact_reference,
        &exact_execution,
    );

    let fallback_reference = run_reference(fallback_case)?;
    let fallback_execution = execute_tassadar_executor_request(
        &fallback_case.program,
        TassadarExecutorDecodeMode::HullCache,
        psionic_runtime::tassadar_trace_abi_for_profile_id(
            fallback_case.program.profile_id.as_str(),
        )
        .expect("article trace ABI should exist")
        .schema_version,
        None,
    )
    .map_err(
        |diagnostic| TassadarExactnessRefusalArtifactError::Execution {
            case_id: fallback_case.case_id.clone(),
            decode_mode: String::from(TassadarExecutorDecodeMode::HullCache.as_str()),
            detail: diagnostic.detail,
        },
    )?;
    let fallback_runtime_report = TassadarExactnessRefusalReport::from_execution_report(
        format!("{}:hull_cache", fallback_case.case_id),
        &fallback_reference,
        &fallback_execution,
    );

    let mut refused_program = exact_case.program.clone();
    refused_program.profile_id = String::from("tassadar.wasm.unsupported_profile.v0");
    let refusal_selection = diagnose_tassadar_executor_request(
        &refused_program,
        TassadarExecutorDecodeMode::ReferenceLinear,
        psionic_runtime::TassadarTraceAbi::article_i32_compute_v1().schema_version,
        None,
    );
    let refusal_runtime_report = TassadarExactnessRefusalReport::from_refusal(
        "unsupported_profile:reference_linear",
        &refusal_selection,
        None,
    );

    Ok(TassadarExactnessRefusalArtifactReport::new(vec![
        TassadarExactnessRefusalArtifactCase {
            case_id: exact_case.case_id.clone(),
            summary: exact_case.summary.clone(),
            runtime_report: exact_runtime_report,
        },
        TassadarExactnessRefusalArtifactCase {
            case_id: fallback_case.case_id.clone(),
            summary: fallback_case.summary.clone(),
            runtime_report: fallback_runtime_report,
        },
        TassadarExactnessRefusalArtifactCase {
            case_id: String::from("unsupported_profile_refusal"),
            summary: String::from(
                "unsupported Wasm-profile refusal over the current exactness/refusal report contract",
            ),
            runtime_report: refusal_runtime_report,
        },
    ]))
}

pub fn tassadar_exactness_refusal_artifact_report_path() -> PathBuf {
    repo_root().join(TASSADAR_EXACTNESS_REFUSAL_REPORT_REF)
}

pub fn write_tassadar_exactness_refusal_artifact_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarExactnessRefusalArtifactReport, TassadarExactnessRefusalArtifactError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarExactnessRefusalArtifactError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_exactness_refusal_artifact_report()?;
    let bytes = serde_json::to_vec_pretty(&report).expect("artifact should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarExactnessRefusalArtifactError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn find_case<'a>(
    cases: &'a [TassadarValidationCase],
    case_id: &str,
) -> Result<&'a TassadarValidationCase, TassadarExactnessRefusalArtifactError> {
    cases
        .iter()
        .find(|case| case.case_id == case_id)
        .ok_or_else(|| TassadarExactnessRefusalArtifactError::MissingCase {
            case_id: String::from(case_id),
        })
}

fn run_reference(
    case: &TassadarValidationCase,
) -> Result<psionic_runtime::TassadarExecution, TassadarExactnessRefusalArtifactError> {
    TassadarCpuReferenceRunner::for_program(&case.program)
        .expect("article corpus should stay reference-runnable")
        .execute(&case.program)
        .map_err(
            |error| TassadarExactnessRefusalArtifactError::ReferenceExecution {
                case_id: case.case_id.clone(),
                error,
            },
        )
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value).expect("exactness/refusal artifact should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_EXACTNESS_REFUSAL_REPORT_REF, TassadarExactnessRefusalArtifactError,
        build_tassadar_exactness_refusal_artifact_report, repo_root,
        tassadar_exactness_refusal_artifact_report_path,
        write_tassadar_exactness_refusal_artifact_report,
    };
    use psionic_runtime::{
        TassadarExactnessPosture, TassadarExecutorSelectionReason, TassadarExecutorSelectionState,
    };
    use tempfile::tempdir;

    #[test]
    fn exactness_refusal_artifact_classifies_exact_fallback_and_refusal_cases()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_exactness_refusal_artifact_report()?;
        assert_eq!(report.cases.len(), 3);

        let exact_case = &report.cases[0];
        assert_eq!(
            exact_case.runtime_report.exactness_posture,
            TassadarExactnessPosture::Exact
        );
        assert_eq!(
            exact_case.runtime_report.selection_state,
            TassadarExecutorSelectionState::Direct
        );

        let fallback_case = &report.cases[1];
        assert_eq!(
            fallback_case.runtime_report.exactness_posture,
            TassadarExactnessPosture::Exact
        );
        assert_eq!(
            fallback_case.runtime_report.selection_state,
            TassadarExecutorSelectionState::Fallback
        );
        assert_eq!(
            fallback_case.runtime_report.selection_reason,
            Some(TassadarExecutorSelectionReason::HullCacheControlFlowUnsupported)
        );

        let refusal_case = &report.cases[2];
        assert_eq!(
            refusal_case.runtime_report.exactness_posture,
            TassadarExactnessPosture::Refused
        );
        assert_eq!(
            refusal_case.runtime_report.selection_state,
            TassadarExecutorSelectionState::Refused
        );
        assert_eq!(
            refusal_case.runtime_report.selection_reason,
            Some(TassadarExecutorSelectionReason::UnsupportedWasmProfile)
        );
        Ok(())
    }

    #[test]
    fn exactness_refusal_artifact_writes_current_truth() -> Result<(), Box<dyn std::error::Error>> {
        let tempdir = tempdir()?;
        let output_path = tempdir
            .path()
            .join("tassadar_exactness_refusal_report.json");
        let written = write_tassadar_exactness_refusal_artifact_report(&output_path)?;
        let decoded = serde_json::from_slice::<super::TassadarExactnessRefusalArtifactReport>(
            &std::fs::read(&output_path)?,
        )?;
        assert_eq!(decoded, written);
        Ok(())
    }

    #[test]
    fn exactness_refusal_artifact_matches_committed_truth() -> Result<(), Box<dyn std::error::Error>>
    {
        let expected = build_tassadar_exactness_refusal_artifact_report()?;
        let committed_path = tassadar_exactness_refusal_artifact_report_path();
        let committed_bytes = std::fs::read(&committed_path).map_err(|error| {
            TassadarExactnessRefusalArtifactError::Read {
                path: committed_path.display().to_string(),
                error,
            }
        })?;
        let committed = serde_json::from_slice::<super::TassadarExactnessRefusalArtifactReport>(
            &committed_bytes,
        )
        .map_err(|error| TassadarExactnessRefusalArtifactError::Decode {
            path: committed_path.display().to_string(),
            error,
        })?;
        assert_eq!(
            committed,
            expected,
            "committed exactness/refusal artifact drifted; rerun `cargo run -p psionic-eval --example tassadar_exactness_refusal_report` from {}",
            tassadar_exactness_refusal_artifact_report_path()
                .strip_prefix(repo_root())?
                .display()
        );
        assert_eq!(
            tassadar_exactness_refusal_artifact_report_path()
                .strip_prefix(repo_root())?
                .display()
                .to_string(),
            TASSADAR_EXACTNESS_REFUSAL_REPORT_REF
        );
        Ok(())
    }
}
