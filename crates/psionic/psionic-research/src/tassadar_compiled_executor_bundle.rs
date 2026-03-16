use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarCompiledExecutorCompatibilityReport, TassadarCompiledExecutorEvalError,
    TassadarCompiledExecutorExactnessReport, build_tassadar_compiled_executor_compatibility_report,
    build_tassadar_compiled_executor_exactness_report,
    build_tassadar_sudoku_v0_compiled_executor_corpus,
};
use psionic_runtime::TassadarExecutorDecodeMode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical output root for the bounded Phase 17 compiled Sudoku-v0 lane.
pub const TASSADAR_COMPILED_EXECUTOR_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/sudoku_v0_compiled_executor_v0";
/// Top-level exactness report for the bounded compiled lane.
pub const TASSADAR_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE: &str =
    "compiled_executor_exactness_report.json";
/// Top-level compatibility/refusal report for the bounded compiled lane.
pub const TASSADAR_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE: &str =
    "compiled_executor_compatibility_report.json";
/// Top-level compiled suite artifact file.
pub const TASSADAR_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE: &str =
    "compiled_weight_suite_artifact.json";

const DEPLOYMENTS_DIR: &str = "deployments";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const PROGRAM_ARTIFACT_FILE: &str = "program_artifact.json";
const COMPILED_WEIGHT_ARTIFACT_FILE: &str = "compiled_weight_artifact.json";
const RUNTIME_CONTRACT_FILE: &str = "runtime_contract.json";
const COMPILED_WEIGHT_BUNDLE_FILE: &str = "compiled_weight_bundle.json";
const COMPILE_EVIDENCE_BUNDLE_FILE: &str = "compile_evidence_bundle.json";
const MODEL_DESCRIPTOR_FILE: &str = "model_descriptor.json";

/// Persisted per-case compiled deployment bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledExecutorDeploymentBundle {
    /// Stable corpus case id.
    pub case_id: String,
    /// Stable split name.
    pub split: String,
    /// Relative deployment directory.
    pub deployment_dir: String,
    /// Relative source program-artifact file.
    pub program_artifact_file: String,
    /// Relative compiled-weight artifact file.
    pub compiled_weight_artifact_file: String,
    /// Relative runtime-contract file.
    pub runtime_contract_file: String,
    /// Relative compiled weight-bundle file.
    pub compiled_weight_bundle_file: String,
    /// Relative compile-evidence-bundle file.
    pub compile_evidence_bundle_file: String,
    /// Relative model-descriptor file.
    pub model_descriptor_file: String,
    /// Stable compiled-weight artifact digest.
    pub compiled_weight_artifact_digest: String,
    /// Stable runtime-contract digest.
    pub runtime_contract_digest: String,
    /// Stable compile proof-bundle digest.
    pub compile_execution_proof_bundle_digest: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl TassadarCompiledExecutorDeploymentBundle {
    fn new(
        case_id: &str,
        split: &str,
        deployment_dir: &str,
        compiled_weight_artifact_digest: String,
        runtime_contract_digest: String,
        compile_execution_proof_bundle_digest: String,
    ) -> Self {
        let mut bundle = Self {
            case_id: case_id.to_string(),
            split: split.to_string(),
            deployment_dir: deployment_dir.to_string(),
            program_artifact_file: String::from(PROGRAM_ARTIFACT_FILE),
            compiled_weight_artifact_file: String::from(COMPILED_WEIGHT_ARTIFACT_FILE),
            runtime_contract_file: String::from(RUNTIME_CONTRACT_FILE),
            compiled_weight_bundle_file: String::from(COMPILED_WEIGHT_BUNDLE_FILE),
            compile_evidence_bundle_file: String::from(COMPILE_EVIDENCE_BUNDLE_FILE),
            model_descriptor_file: String::from(MODEL_DESCRIPTOR_FILE),
            compiled_weight_artifact_digest,
            runtime_contract_digest,
            compile_execution_proof_bundle_digest,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_compiled_executor_deployment_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Top-level persisted bundle for the bounded Phase 17 compiled Sudoku-v0 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledExecutorRunBundle {
    /// Stable run id.
    pub run_id: String,
    /// Stable workload family id.
    pub workload_family_id: String,
    /// Explicit claim boundary.
    pub claim_boundary: String,
    /// Serving posture for the lane.
    pub serve_posture: String,
    /// Requested decode mode used for the exactness run.
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    /// Relative exactness report file.
    pub exactness_report_file: String,
    /// Relative compatibility/refusal report file.
    pub compatibility_report_file: String,
    /// Relative compiled suite-artifact file.
    pub compiled_suite_artifact_file: String,
    /// Ordered per-case deployment bundles.
    pub deployments: Vec<TassadarCompiledExecutorDeploymentBundle>,
    /// Stable exactness-report digest.
    pub exactness_report_digest: String,
    /// Stable compatibility-report digest.
    pub compatibility_report_digest: String,
    /// Stable compiled suite-artifact digest.
    pub compiled_suite_artifact_digest: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl TassadarCompiledExecutorRunBundle {
    fn new(
        exactness_report: &TassadarCompiledExecutorExactnessReport,
        compatibility_report: &TassadarCompiledExecutorCompatibilityReport,
        compiled_suite_artifact_digest: String,
        deployments: Vec<TassadarCompiledExecutorDeploymentBundle>,
    ) -> Self {
        let mut bundle = Self {
            run_id: String::from("tassadar-sudoku-v0-compiled-executor-v0"),
            workload_family_id: exactness_report.workload_family_id.clone(),
            claim_boundary: String::from(
                "bounded compiled/proof-backed Sudoku-v0 executor lane exact on the matched corpus; not arbitrary-program closure and not exposed in serving by default",
            ),
            serve_posture: String::from("eval_only"),
            requested_decode_mode: exactness_report.requested_decode_mode,
            exactness_report_file: String::from(TASSADAR_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE),
            compatibility_report_file: String::from(
                TASSADAR_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE,
            ),
            compiled_suite_artifact_file: String::from(
                TASSADAR_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE,
            ),
            deployments,
            exactness_report_digest: exactness_report.report_digest.clone(),
            compatibility_report_digest: compatibility_report.report_digest.clone(),
            compiled_suite_artifact_digest,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest =
            stable_digest(b"psionic_tassadar_compiled_executor_run_bundle|", &bundle);
        bundle
    }
}

/// Errors while writing the bounded Phase 17 compiled Sudoku-v0 bundle.
#[derive(Debug, Error)]
pub enum TassadarCompiledExecutorPersistError {
    /// Building or evaluating the compiled lane failed.
    #[error(transparent)]
    Eval(#[from] TassadarCompiledExecutorEvalError),
    /// Creating one output directory failed.
    #[error("failed to create `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
    /// Writing one artifact failed.
    #[error("failed to write `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// Source error.
        error: std::io::Error,
    },
}

/// Executes the bounded Phase 17 compiled Sudoku-v0 lane and writes the
/// resulting bundle plus exactness/refusal artifacts.
pub fn run_tassadar_compiled_executor_bundle(
    output_dir: &Path,
) -> Result<TassadarCompiledExecutorRunBundle, TassadarCompiledExecutorPersistError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarCompiledExecutorPersistError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let corpus = build_tassadar_sudoku_v0_compiled_executor_corpus(None)?;
    let exactness_report = build_tassadar_compiled_executor_exactness_report(
        &corpus,
        TassadarExecutorDecodeMode::ReferenceLinear,
    )?;
    let compatibility_report = build_tassadar_compiled_executor_compatibility_report(&corpus)?;

    write_json(
        output_dir.join(TASSADAR_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE),
        &exactness_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE),
        &compatibility_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE),
        &corpus.compiled_suite_artifact,
    )?;

    let deployments = persist_deployments(output_dir, &corpus)?;
    let bundle = TassadarCompiledExecutorRunBundle::new(
        &exactness_report,
        &compatibility_report,
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        deployments,
    );
    write_json(output_dir.join(RUN_BUNDLE_FILE), &bundle)?;
    Ok(bundle)
}

fn persist_deployments(
    output_dir: &Path,
    corpus: &psionic_eval::TassadarCompiledExecutorCorpus,
) -> Result<Vec<TassadarCompiledExecutorDeploymentBundle>, TassadarCompiledExecutorPersistError> {
    let deployments_root = output_dir.join(DEPLOYMENTS_DIR);
    fs::create_dir_all(&deployments_root).map_err(|error| {
        TassadarCompiledExecutorPersistError::CreateDir {
            path: deployments_root.display().to_string(),
            error,
        }
    })?;

    let mut bundles = Vec::with_capacity(corpus.cases.len());
    for case in &corpus.cases {
        let deployment_dir = deployments_root.join(case.case_id.as_str());
        let relative_deployment_dir = PathBuf::from(DEPLOYMENTS_DIR)
            .join(case.case_id.as_str())
            .display()
            .to_string();
        fs::create_dir_all(&deployment_dir).map_err(|error| {
            TassadarCompiledExecutorPersistError::CreateDir {
                path: deployment_dir.display().to_string(),
                error,
            }
        })?;
        write_json(
            deployment_dir.join(PROGRAM_ARTIFACT_FILE),
            &case.program_artifact,
        )?;
        write_json(
            deployment_dir.join(COMPILED_WEIGHT_ARTIFACT_FILE),
            case.compiled_executor.compiled_weight_artifact(),
        )?;
        write_json(
            deployment_dir.join(RUNTIME_CONTRACT_FILE),
            case.compiled_executor.runtime_contract(),
        )?;
        write_json(
            deployment_dir.join(COMPILED_WEIGHT_BUNDLE_FILE),
            case.compiled_executor.weight_bundle(),
        )?;
        write_json(
            deployment_dir.join(COMPILE_EVIDENCE_BUNDLE_FILE),
            case.compiled_executor.compile_evidence_bundle(),
        )?;
        write_json(
            deployment_dir.join(MODEL_DESCRIPTOR_FILE),
            case.compiled_executor.descriptor(),
        )?;

        bundles.push(TassadarCompiledExecutorDeploymentBundle::new(
            case.case_id.as_str(),
            case.split.as_str(),
            relative_deployment_dir.as_str(),
            case.compiled_executor
                .compiled_weight_artifact()
                .artifact_digest
                .clone(),
            case.compiled_executor
                .runtime_contract()
                .contract_digest
                .clone(),
            case.compiled_executor
                .compile_evidence_bundle()
                .proof_bundle
                .stable_digest(),
        ));
    }
    Ok(bundles)
}

fn write_json<T>(
    path: impl AsRef<Path>,
    value: &T,
) -> Result<(), TassadarCompiledExecutorPersistError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value)
        .expect("Tassadar compiled executor bundle artifact should serialize");
    fs::write(path, &bytes).map_err(|error| TassadarCompiledExecutorPersistError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar compiled executor bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        RUN_BUNDLE_FILE, TASSADAR_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE,
        TASSADAR_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE,
        TASSADAR_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE, run_tassadar_compiled_executor_bundle,
    };

    #[test]
    fn compiled_executor_bundle_writes_reports_and_deployments()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bundle = run_tassadar_compiled_executor_bundle(temp.path())?;

        assert_eq!(bundle.deployments.len(), 8);
        assert!(
            temp.path()
                .join(TASSADAR_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE)
                .exists()
        );
        assert!(temp.path().join(RUN_BUNDLE_FILE).exists());
        assert!(
            temp.path()
                .join("deployments")
                .join("sudoku_v0_validation_a")
                .join("compile_evidence_bundle.json")
                .exists()
        );
        Ok(())
    }
}
