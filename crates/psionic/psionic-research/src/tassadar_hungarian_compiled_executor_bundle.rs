use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarHungarianCompiledExecutorCompatibilityReport, TassadarHungarianCompiledExecutorCorpus,
    TassadarHungarianCompiledExecutorEvalError, TassadarHungarianCompiledExecutorExactnessReport,
    TassadarHungarianLaneStatusReport, TassadarReferenceFixtureSuite,
    build_tassadar_hungarian_compiled_executor_compatibility_report,
    build_tassadar_hungarian_compiled_executor_exactness_report,
    build_tassadar_hungarian_lane_status_report,
    build_tassadar_hungarian_v0_compiled_executor_corpus, build_tassadar_hungarian_v0_suite,
};
use psionic_runtime::TassadarExecutorDecodeMode;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Canonical output root for the bounded Phase 18 compiled Hungarian-v0 lane.
pub const TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_OUTPUT_DIR: &str =
    "crates/psionic/fixtures/tassadar/runs/hungarian_v0_compiled_executor_v0";
/// Top-level exactness report for the bounded compiled lane.
pub const TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE: &str =
    "compiled_executor_exactness_report.json";
/// Top-level compatibility/refusal report for the bounded compiled lane.
pub const TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE: &str =
    "compiled_executor_compatibility_report.json";
/// Top-level lane-status report separating learned from compiled claims.
pub const TASSADAR_HUNGARIAN_LANE_STATUS_REPORT_FILE: &str = "hungarian_lane_status_report.json";
/// Top-level environment bundle file.
pub const TASSADAR_HUNGARIAN_ENVIRONMENT_BUNDLE_FILE: &str = "environment_bundle.json";
/// Top-level benchmark package file.
pub const TASSADAR_HUNGARIAN_BENCHMARK_PACKAGE_FILE: &str = "benchmark_package.json";
/// Top-level compiled suite artifact file.
pub const TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE: &str =
    "compiled_weight_suite_artifact.json";

const DEPLOYMENTS_DIR: &str = "deployments";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const PROGRAM_ARTIFACT_FILE: &str = "program_artifact.json";
const COMPILED_WEIGHT_ARTIFACT_FILE: &str = "compiled_weight_artifact.json";
const RUNTIME_CONTRACT_FILE: &str = "runtime_contract.json";
const COMPILED_WEIGHT_BUNDLE_FILE: &str = "compiled_weight_bundle.json";
const COMPILE_EVIDENCE_BUNDLE_FILE: &str = "compile_evidence_bundle.json";
const MODEL_DESCRIPTOR_FILE: &str = "model_descriptor.json";

/// Persisted per-case compiled deployment bundle for one Hungarian-v0 program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianCompiledExecutorDeploymentBundle {
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

impl TassadarHungarianCompiledExecutorDeploymentBundle {
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
            b"psionic_tassadar_hungarian_compiled_executor_deployment_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Top-level persisted bundle for the bounded Phase 18 compiled Hungarian-v0 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarHungarianCompiledExecutorRunBundle {
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
    /// Relative benchmark package file.
    pub benchmark_package_file: String,
    /// Relative environment bundle file.
    pub environment_bundle_file: String,
    /// Relative exactness report file.
    pub exactness_report_file: String,
    /// Relative compatibility/refusal report file.
    pub compatibility_report_file: String,
    /// Relative lane-status report file.
    pub lane_status_report_file: String,
    /// Relative compiled suite-artifact file.
    pub compiled_suite_artifact_file: String,
    /// Ordered per-case deployment bundles.
    pub deployments: Vec<TassadarHungarianCompiledExecutorDeploymentBundle>,
    /// Stable benchmark package digest.
    pub benchmark_package_digest: String,
    /// Stable environment bundle digest.
    pub environment_bundle_digest: String,
    /// Stable exactness-report digest.
    pub exactness_report_digest: String,
    /// Stable compatibility-report digest.
    pub compatibility_report_digest: String,
    /// Stable lane-status report digest.
    pub lane_status_report_digest: String,
    /// Stable compiled suite-artifact digest.
    pub compiled_suite_artifact_digest: String,
    /// Stable bundle digest.
    pub bundle_digest: String,
}

impl TassadarHungarianCompiledExecutorRunBundle {
    fn new(
        suite: &TassadarReferenceFixtureSuite,
        exactness_report: &TassadarHungarianCompiledExecutorExactnessReport,
        compatibility_report: &TassadarHungarianCompiledExecutorCompatibilityReport,
        lane_status_report: &TassadarHungarianLaneStatusReport,
        compiled_suite_artifact_digest: String,
        deployments: Vec<TassadarHungarianCompiledExecutorDeploymentBundle>,
    ) -> Self {
        let mut bundle = Self {
            run_id: String::from("tassadar-hungarian-v0-compiled-executor-v0"),
            workload_family_id: exactness_report.workload_family_id.clone(),
            claim_boundary: String::from(
                "bounded compiled/proof-backed Hungarian-v0 lane exact on the matched 4x4 corpus and benchmark package; not a learned lane, not arbitrary-program closure, and not article parity",
            ),
            serve_posture: String::from("eval_only"),
            requested_decode_mode: exactness_report.requested_decode_mode,
            benchmark_package_file: String::from(TASSADAR_HUNGARIAN_BENCHMARK_PACKAGE_FILE),
            environment_bundle_file: String::from(TASSADAR_HUNGARIAN_ENVIRONMENT_BUNDLE_FILE),
            exactness_report_file: String::from(
                TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE,
            ),
            compatibility_report_file: String::from(
                TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE,
            ),
            lane_status_report_file: String::from(TASSADAR_HUNGARIAN_LANE_STATUS_REPORT_FILE),
            compiled_suite_artifact_file: String::from(
                TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE,
            ),
            deployments,
            benchmark_package_digest: suite.benchmark_package.stable_digest(),
            environment_bundle_digest: stable_digest(
                b"psionic_tassadar_hungarian_environment_bundle|",
                &suite.environment_bundle,
            ),
            exactness_report_digest: exactness_report.report_digest.clone(),
            compatibility_report_digest: compatibility_report.report_digest.clone(),
            lane_status_report_digest: lane_status_report.report_digest.clone(),
            compiled_suite_artifact_digest,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_hungarian_compiled_executor_run_bundle|",
            &bundle,
        );
        bundle
    }
}

/// Errors while writing the bounded Phase 18 Hungarian bundle.
#[derive(Debug, Error)]
pub enum TassadarHungarianCompiledExecutorPersistError {
    /// Building or evaluating the compiled lane failed.
    #[error(transparent)]
    Eval(#[from] TassadarHungarianCompiledExecutorEvalError),
    /// Building the benchmark/environment package failed.
    #[error(transparent)]
    Benchmark(#[from] psionic_eval::TassadarBenchmarkError),
    /// The benchmark package and compiled lane did not target the same program digests.
    #[error(
        "benchmark/compiled program digest mismatch for `{case_id}`: benchmark `{benchmark_program_digest}` vs compiled `{compiled_program_digest}`"
    )]
    ProgramDigestMismatch {
        /// Stable case id.
        case_id: String,
        /// Benchmark-program digest.
        benchmark_program_digest: String,
        /// Compiled-program digest.
        compiled_program_digest: String,
    },
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

/// Executes the bounded Phase 18 compiled Hungarian-v0 lane and writes the
/// resulting bundle plus benchmark/exactness/refusal artifacts.
pub fn run_tassadar_hungarian_compiled_executor_bundle(
    output_dir: &Path,
) -> Result<TassadarHungarianCompiledExecutorRunBundle, TassadarHungarianCompiledExecutorPersistError>
{
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarHungarianCompiledExecutorPersistError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let suite = build_tassadar_hungarian_v0_suite("v0")?;
    let corpus = build_tassadar_hungarian_v0_compiled_executor_corpus(None)?;
    assert_program_digest_alignment(&suite, &corpus)?;
    let exactness_report = build_tassadar_hungarian_compiled_executor_exactness_report(
        &corpus,
        TassadarExecutorDecodeMode::ReferenceLinear,
    )?;
    let compatibility_report =
        build_tassadar_hungarian_compiled_executor_compatibility_report(&corpus)?;
    let lane_status_report = build_tassadar_hungarian_lane_status_report();

    write_json(
        output_dir.join(TASSADAR_HUNGARIAN_BENCHMARK_PACKAGE_FILE),
        &suite.benchmark_package,
    )?;
    write_json(
        output_dir.join(TASSADAR_HUNGARIAN_ENVIRONMENT_BUNDLE_FILE),
        &suite.environment_bundle,
    )?;
    write_json(
        output_dir.join(TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE),
        &exactness_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE),
        &compatibility_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_HUNGARIAN_LANE_STATUS_REPORT_FILE),
        &lane_status_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE),
        &corpus.compiled_suite_artifact,
    )?;

    let deployments = persist_deployments(output_dir, &corpus)?;
    let bundle = TassadarHungarianCompiledExecutorRunBundle::new(
        &suite,
        &exactness_report,
        &compatibility_report,
        &lane_status_report,
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        deployments,
    );
    write_json(output_dir.join(RUN_BUNDLE_FILE), &bundle)?;
    Ok(bundle)
}

fn assert_program_digest_alignment(
    suite: &TassadarReferenceFixtureSuite,
    corpus: &TassadarHungarianCompiledExecutorCorpus,
) -> Result<(), TassadarHungarianCompiledExecutorPersistError> {
    let benchmark_digests = suite
        .artifacts
        .iter()
        .map(|artifact| {
            (
                artifact.validated_program.program_id.clone(),
                artifact.validated_program_digest.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let compiled_digests = corpus
        .cases
        .iter()
        .map(|case| {
            (
                case.program_artifact.validated_program.program_id.clone(),
                case.program_artifact.validated_program_digest.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (case_id, benchmark_program_digest) in benchmark_digests {
        let Some(compiled_program_digest) = compiled_digests.get(&case_id) else {
            return Err(
                TassadarHungarianCompiledExecutorPersistError::ProgramDigestMismatch {
                    case_id,
                    benchmark_program_digest,
                    compiled_program_digest: String::from("missing"),
                },
            );
        };
        if compiled_program_digest != &benchmark_program_digest {
            return Err(
                TassadarHungarianCompiledExecutorPersistError::ProgramDigestMismatch {
                    case_id,
                    benchmark_program_digest,
                    compiled_program_digest: compiled_program_digest.clone(),
                },
            );
        }
    }

    Ok(())
}

fn persist_deployments(
    output_dir: &Path,
    corpus: &TassadarHungarianCompiledExecutorCorpus,
) -> Result<
    Vec<TassadarHungarianCompiledExecutorDeploymentBundle>,
    TassadarHungarianCompiledExecutorPersistError,
> {
    let deployments_root = output_dir.join(DEPLOYMENTS_DIR);
    fs::create_dir_all(&deployments_root).map_err(|error| {
        TassadarHungarianCompiledExecutorPersistError::CreateDir {
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
            TassadarHungarianCompiledExecutorPersistError::CreateDir {
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

        bundles.push(TassadarHungarianCompiledExecutorDeploymentBundle::new(
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
) -> Result<(), TassadarHungarianCompiledExecutorPersistError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value)
        .expect("Tassadar Hungarian compiled executor bundle artifact should serialize");
    fs::write(path, &bytes).map_err(
        |error| TassadarHungarianCompiledExecutorPersistError::Write {
            path: path.display().to_string(),
            error,
        },
    )
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = serde_json::to_vec(value)
        .expect("Tassadar Hungarian compiled executor bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        RUN_BUNDLE_FILE, TASSADAR_HUNGARIAN_BENCHMARK_PACKAGE_FILE,
        TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE,
        TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE,
        TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE,
        TASSADAR_HUNGARIAN_ENVIRONMENT_BUNDLE_FILE, TASSADAR_HUNGARIAN_LANE_STATUS_REPORT_FILE,
        run_tassadar_hungarian_compiled_executor_bundle,
    };

    #[test]
    fn compiled_hungarian_executor_bundle_writes_reports_and_deployments()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bundle = run_tassadar_hungarian_compiled_executor_bundle(temp.path())?;

        assert_eq!(bundle.deployments.len(), 8);
        assert!(
            temp.path()
                .join(TASSADAR_HUNGARIAN_BENCHMARK_PACKAGE_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_HUNGARIAN_ENVIRONMENT_BUNDLE_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_EXACTNESS_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_COMPATIBILITY_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_HUNGARIAN_LANE_STATUS_REPORT_FILE)
                .exists()
        );
        assert!(
            temp.path()
                .join(TASSADAR_HUNGARIAN_COMPILED_EXECUTOR_SUITE_ARTIFACT_FILE)
                .exists()
        );
        assert!(temp.path().join(RUN_BUNDLE_FILE).exists());
        assert!(
            temp.path()
                .join("deployments")
                .join("hungarian_v0_validation_a")
                .join("compile_evidence_bundle.json")
                .exists()
        );
        Ok(())
    }
}
