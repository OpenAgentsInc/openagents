use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_tassadar_compiled_kernel_suite,
    build_tassadar_compiled_kernel_suite_claim_boundary_report,
    build_tassadar_compiled_kernel_suite_compatibility_report,
    build_tassadar_compiled_kernel_suite_corpus,
    build_tassadar_compiled_kernel_suite_exactness_report,
    build_tassadar_compiled_kernel_suite_scaling_report,
    TassadarCompiledKernelFamilyId, TassadarCompiledKernelSuiteCaseExactnessReport,
    TassadarCompiledKernelSuiteClaimBoundaryReport,
    TassadarCompiledKernelSuiteCompatibilityReport, TassadarCompiledKernelSuiteCorpus,
    TassadarCompiledKernelSuiteEvalError, TassadarCompiledKernelSuiteExactnessReport,
    TassadarCompiledKernelSuiteScalingReport, TassadarReferenceFixtureSuite,
};
use psionic_runtime::{TassadarClaimClass, TassadarExecutorDecodeMode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_COMPILED_KERNEL_SUITE_OUTPUT_DIR: &str =
    "fixtures/tassadar/runs/compiled_kernel_suite_v0";
pub const TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_PACKAGE_FILE: &str = "benchmark_package.json";
pub const TASSADAR_COMPILED_KERNEL_SUITE_ENVIRONMENT_BUNDLE_FILE: &str = "environment_bundle.json";
pub const TASSADAR_COMPILED_KERNEL_SUITE_EXACTNESS_REPORT_FILE: &str =
    "compiled_kernel_suite_exactness_report.json";
pub const TASSADAR_COMPILED_KERNEL_SUITE_COMPATIBILITY_REPORT_FILE: &str =
    "compiled_kernel_suite_compatibility_report.json";
pub const TASSADAR_COMPILED_KERNEL_SUITE_SCALING_REPORT_FILE: &str =
    "compiled_kernel_suite_scaling_report.json";
pub const TASSADAR_COMPILED_KERNEL_SUITE_CLAIM_BOUNDARY_REPORT_FILE: &str =
    "claim_boundary_report.json";
pub const TASSADAR_COMPILED_KERNEL_SUITE_ARTIFACT_FILE: &str =
    "compiled_weight_suite_artifact.json";

const DEPLOYMENTS_DIR: &str = "deployments";
const RUN_BUNDLE_FILE: &str = "run_bundle.json";
const PROGRAM_ARTIFACT_FILE: &str = "program_artifact.json";
const COMPILED_WEIGHT_ARTIFACT_FILE: &str = "compiled_weight_artifact.json";
const RUNTIME_CONTRACT_FILE: &str = "runtime_contract.json";
const COMPILED_WEIGHT_BUNDLE_FILE: &str = "compiled_weight_bundle.json";
const COMPILE_EVIDENCE_BUNDLE_FILE: &str = "compile_evidence_bundle.json";
const MODEL_DESCRIPTOR_FILE: &str = "model_descriptor.json";
const RUNTIME_EXECUTION_PROOF_BUNDLE_FILE: &str = "runtime_execution_proof_bundle.json";
const RUNTIME_TRACE_PROOF_FILE: &str = "runtime_trace_proof.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteDeploymentBundle {
    pub case_id: String,
    pub family_id: TassadarCompiledKernelFamilyId,
    pub regime_id: String,
    pub deployment_dir: String,
    pub program_artifact_file: String,
    pub compiled_weight_artifact_file: String,
    pub runtime_contract_file: String,
    pub compiled_weight_bundle_file: String,
    pub compile_evidence_bundle_file: String,
    pub model_descriptor_file: String,
    pub runtime_execution_proof_bundle_file: String,
    pub runtime_trace_proof_file: String,
    pub compiled_weight_artifact_digest: String,
    pub runtime_contract_digest: String,
    pub compile_execution_proof_bundle_digest: String,
    pub runtime_execution_proof_bundle_digest: String,
    pub runtime_trace_proof_digest: String,
    pub bundle_digest: String,
}

impl TassadarCompiledKernelSuiteDeploymentBundle {
    fn new(
        case_id: &str,
        family_id: TassadarCompiledKernelFamilyId,
        regime_id: &str,
        deployment_dir: &str,
        compiled_weight_artifact_digest: String,
        runtime_contract_digest: String,
        compile_execution_proof_bundle_digest: String,
        runtime_execution_proof_bundle_digest: String,
        runtime_trace_proof_digest: String,
    ) -> Self {
        let mut bundle = Self {
            case_id: case_id.to_string(),
            family_id,
            regime_id: regime_id.to_string(),
            deployment_dir: deployment_dir.to_string(),
            program_artifact_file: String::from(PROGRAM_ARTIFACT_FILE),
            compiled_weight_artifact_file: String::from(COMPILED_WEIGHT_ARTIFACT_FILE),
            runtime_contract_file: String::from(RUNTIME_CONTRACT_FILE),
            compiled_weight_bundle_file: String::from(COMPILED_WEIGHT_BUNDLE_FILE),
            compile_evidence_bundle_file: String::from(COMPILE_EVIDENCE_BUNDLE_FILE),
            model_descriptor_file: String::from(MODEL_DESCRIPTOR_FILE),
            runtime_execution_proof_bundle_file: String::from(RUNTIME_EXECUTION_PROOF_BUNDLE_FILE),
            runtime_trace_proof_file: String::from(RUNTIME_TRACE_PROOF_FILE),
            compiled_weight_artifact_digest,
            runtime_contract_digest,
            compile_execution_proof_bundle_digest,
            runtime_execution_proof_bundle_digest,
            runtime_trace_proof_digest,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_compiled_kernel_suite_deployment_bundle|",
            &bundle,
        );
        bundle
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarCompiledKernelSuiteRunBundle {
    pub run_id: String,
    pub workload_family_id: String,
    pub claim_class: TassadarClaimClass,
    pub claim_boundary: String,
    pub serve_posture: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub benchmark_package_file: String,
    pub environment_bundle_file: String,
    pub exactness_report_file: String,
    pub compatibility_report_file: String,
    pub scaling_report_file: String,
    pub claim_boundary_report_file: String,
    pub compiled_suite_artifact_file: String,
    pub deployments: Vec<TassadarCompiledKernelSuiteDeploymentBundle>,
    pub benchmark_package_digest: String,
    pub environment_bundle_digest: String,
    pub exactness_report_digest: String,
    pub compatibility_report_digest: String,
    pub scaling_report_digest: String,
    pub claim_boundary_report_digest: String,
    pub compiled_suite_artifact_digest: String,
    pub bundle_digest: String,
}

impl TassadarCompiledKernelSuiteRunBundle {
    fn new(
        suite: &TassadarReferenceFixtureSuite,
        exactness_report: &TassadarCompiledKernelSuiteExactnessReport,
        compatibility_report: &TassadarCompiledKernelSuiteCompatibilityReport,
        scaling_report: &TassadarCompiledKernelSuiteScalingReport,
        claim_boundary_report: &TassadarCompiledKernelSuiteClaimBoundaryReport,
        compiled_suite_artifact_digest: String,
        deployments: Vec<TassadarCompiledKernelSuiteDeploymentBundle>,
    ) -> Self {
        let mut bundle = Self {
            run_id: String::from("tassadar-compiled-kernel-suite-v0"),
            workload_family_id: exactness_report.workload_family_id.clone(),
            claim_class: TassadarClaimClass::CompiledArticleClass,
            claim_boundary: String::from(
                "exact compiled/proof-backed generic kernel suite over bounded arithmetic, memory-update, forward-branch, and backward-loop families under the article_i32 profile; this widens compiled article closure evidence beyond Sudoku and Hungarian but is not arbitrary-program closure by itself",
            ),
            serve_posture: String::from("eval_only"),
            requested_decode_mode: exactness_report.requested_decode_mode,
            benchmark_package_file: String::from(TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_PACKAGE_FILE),
            environment_bundle_file: String::from(
                TASSADAR_COMPILED_KERNEL_SUITE_ENVIRONMENT_BUNDLE_FILE,
            ),
            exactness_report_file: String::from(TASSADAR_COMPILED_KERNEL_SUITE_EXACTNESS_REPORT_FILE),
            compatibility_report_file: String::from(
                TASSADAR_COMPILED_KERNEL_SUITE_COMPATIBILITY_REPORT_FILE,
            ),
            scaling_report_file: String::from(TASSADAR_COMPILED_KERNEL_SUITE_SCALING_REPORT_FILE),
            claim_boundary_report_file: String::from(
                TASSADAR_COMPILED_KERNEL_SUITE_CLAIM_BOUNDARY_REPORT_FILE,
            ),
            compiled_suite_artifact_file: String::from(TASSADAR_COMPILED_KERNEL_SUITE_ARTIFACT_FILE),
            deployments,
            benchmark_package_digest: suite.benchmark_package.stable_digest(),
            environment_bundle_digest: stable_digest(
                b"psionic_tassadar_compiled_kernel_suite_environment_bundle|",
                &suite.environment_bundle,
            ),
            exactness_report_digest: exactness_report.report_digest.clone(),
            compatibility_report_digest: compatibility_report.report_digest.clone(),
            scaling_report_digest: scaling_report.report_digest.clone(),
            claim_boundary_report_digest: claim_boundary_report.report_digest.clone(),
            compiled_suite_artifact_digest,
            bundle_digest: String::new(),
        };
        bundle.bundle_digest = stable_digest(
            b"psionic_tassadar_compiled_kernel_suite_run_bundle|",
            &bundle,
        );
        bundle
    }
}

#[derive(Debug, Error)]
pub enum TassadarCompiledKernelSuitePersistError {
    #[error(transparent)]
    Eval(#[from] TassadarCompiledKernelSuiteEvalError),
    #[error(transparent)]
    Compiled(#[from] psionic_models::TassadarCompiledProgramError),
    #[error(
        "benchmark/compiled program digest mismatch for `{case_id}`: benchmark `{benchmark_program_digest}` vs compiled `{compiled_program_digest}`"
    )]
    ProgramDigestMismatch {
        case_id: String,
        benchmark_program_digest: String,
        compiled_program_digest: String,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

pub fn run_tassadar_compiled_kernel_suite_bundle(
    output_dir: &Path,
) -> Result<TassadarCompiledKernelSuiteRunBundle, TassadarCompiledKernelSuitePersistError> {
    fs::create_dir_all(output_dir).map_err(|error| {
        TassadarCompiledKernelSuitePersistError::CreateDir {
            path: output_dir.display().to_string(),
            error,
        }
    })?;

    let suite = build_tassadar_compiled_kernel_suite("v0")?;
    let corpus = build_tassadar_compiled_kernel_suite_corpus()?;
    assert_program_digest_alignment(&suite, &corpus)?;
    let exactness_report = build_tassadar_compiled_kernel_suite_exactness_report(
        &corpus,
        TassadarExecutorDecodeMode::ReferenceLinear,
    )?;
    let compatibility_report =
        build_tassadar_compiled_kernel_suite_compatibility_report(&corpus)?;
    let scaling_report = build_tassadar_compiled_kernel_suite_scaling_report(
        &corpus,
        TassadarExecutorDecodeMode::ReferenceLinear,
    )?;
    let claim_boundary_report = build_tassadar_compiled_kernel_suite_claim_boundary_report();

    write_json(
        output_dir.join(TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_PACKAGE_FILE),
        &suite.benchmark_package,
    )?;
    write_json(
        output_dir.join(TASSADAR_COMPILED_KERNEL_SUITE_ENVIRONMENT_BUNDLE_FILE),
        &suite.environment_bundle,
    )?;
    write_json(
        output_dir.join(TASSADAR_COMPILED_KERNEL_SUITE_EXACTNESS_REPORT_FILE),
        &exactness_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_COMPILED_KERNEL_SUITE_COMPATIBILITY_REPORT_FILE),
        &compatibility_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_COMPILED_KERNEL_SUITE_SCALING_REPORT_FILE),
        &scaling_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_COMPILED_KERNEL_SUITE_CLAIM_BOUNDARY_REPORT_FILE),
        &claim_boundary_report,
    )?;
    write_json(
        output_dir.join(TASSADAR_COMPILED_KERNEL_SUITE_ARTIFACT_FILE),
        &corpus.compiled_suite_artifact,
    )?;

    let exactness_lookup = exactness_report
        .case_reports
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect::<BTreeMap<_, _>>();
    let deployments = persist_deployments(output_dir, &corpus, &exactness_lookup)?;
    let bundle = TassadarCompiledKernelSuiteRunBundle::new(
        &suite,
        &exactness_report,
        &compatibility_report,
        &scaling_report,
        &claim_boundary_report,
        corpus.compiled_suite_artifact.artifact_digest.clone(),
        deployments,
    );
    write_json(output_dir.join(RUN_BUNDLE_FILE), &bundle)?;
    Ok(bundle)
}

fn assert_program_digest_alignment(
    suite: &TassadarReferenceFixtureSuite,
    corpus: &TassadarCompiledKernelSuiteCorpus,
) -> Result<(), TassadarCompiledKernelSuitePersistError> {
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
            return Err(TassadarCompiledKernelSuitePersistError::ProgramDigestMismatch {
                case_id,
                benchmark_program_digest,
                compiled_program_digest: String::from("missing"),
            });
        };
        if compiled_program_digest != &benchmark_program_digest {
            return Err(TassadarCompiledKernelSuitePersistError::ProgramDigestMismatch {
                case_id,
                benchmark_program_digest,
                compiled_program_digest: compiled_program_digest.clone(),
            });
        }
    }

    Ok(())
}

fn persist_deployments(
    output_dir: &Path,
    corpus: &TassadarCompiledKernelSuiteCorpus,
    exactness_lookup: &BTreeMap<&str, &TassadarCompiledKernelSuiteCaseExactnessReport>,
) -> Result<Vec<TassadarCompiledKernelSuiteDeploymentBundle>, TassadarCompiledKernelSuitePersistError>
{
    let deployments_root = output_dir.join(DEPLOYMENTS_DIR);
    fs::create_dir_all(&deployments_root).map_err(|error| {
        TassadarCompiledKernelSuitePersistError::CreateDir {
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
            TassadarCompiledKernelSuitePersistError::CreateDir {
                path: deployment_dir.display().to_string(),
                error,
            }
        })?;

        let execution = case
            .compiled_executor
            .execute(&case.program_artifact, TassadarExecutorDecodeMode::ReferenceLinear)?;

        write_json(deployment_dir.join(PROGRAM_ARTIFACT_FILE), &case.program_artifact)?;
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
        write_json(
            deployment_dir.join(RUNTIME_EXECUTION_PROOF_BUNDLE_FILE),
            &execution.evidence_bundle.proof_bundle,
        )?;
        write_json(
            deployment_dir.join(RUNTIME_TRACE_PROOF_FILE),
            &execution.evidence_bundle.trace_proof,
        )?;

        let exact_case = exactness_lookup
            .get(case.case_id.as_str())
            .expect("exactness report should cover each compiled kernel case");
        bundles.push(TassadarCompiledKernelSuiteDeploymentBundle::new(
            case.case_id.as_str(),
            case.family_id,
            case.regime_id.as_str(),
            relative_deployment_dir.as_str(),
            case.compiled_executor
                .compiled_weight_artifact()
                .artifact_digest
                .clone(),
            case.compiled_executor.runtime_contract().contract_digest.clone(),
            case.compiled_executor
                .compile_evidence_bundle()
                .proof_bundle
                .stable_digest(),
            exact_case.runtime_execution_proof_bundle_digest.clone(),
            execution.evidence_bundle.trace_proof.proof_digest.clone(),
        ));
    }
    Ok(bundles)
}

fn write_json<T>(
    path: impl AsRef<Path>,
    value: &T,
) -> Result<(), TassadarCompiledKernelSuitePersistError>
where
    T: Serialize,
{
    let path = path.as_ref();
    let bytes = serde_json::to_vec_pretty(value)
        .expect("Tassadar compiled kernel suite artifact should serialize");
    fs::write(path, &bytes).map_err(|error| TassadarCompiledKernelSuitePersistError::Write {
        path: path.display().to_string(),
        error,
    })
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded =
        serde_json::to_vec(value).expect("Tassadar compiled kernel suite bundle should serialize");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        run_tassadar_compiled_kernel_suite_bundle, RUN_BUNDLE_FILE,
        TASSADAR_COMPILED_KERNEL_SUITE_ARTIFACT_FILE,
        TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_PACKAGE_FILE,
        TASSADAR_COMPILED_KERNEL_SUITE_CLAIM_BOUNDARY_REPORT_FILE,
        TASSADAR_COMPILED_KERNEL_SUITE_COMPATIBILITY_REPORT_FILE,
        TASSADAR_COMPILED_KERNEL_SUITE_ENVIRONMENT_BUNDLE_FILE,
        TASSADAR_COMPILED_KERNEL_SUITE_EXACTNESS_REPORT_FILE,
        TASSADAR_COMPILED_KERNEL_SUITE_SCALING_REPORT_FILE,
    };
    use psionic_runtime::TassadarClaimClass;

    #[test]
    fn compiled_kernel_suite_bundle_writes_reports_and_deployments(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let bundle = run_tassadar_compiled_kernel_suite_bundle(temp.path())?;

        assert_eq!(bundle.claim_class, TassadarClaimClass::CompiledArticleClass);
        assert_eq!(bundle.deployments.len(), 12);
        assert!(temp
            .path()
            .join(TASSADAR_COMPILED_KERNEL_SUITE_BENCHMARK_PACKAGE_FILE)
            .exists());
        assert!(temp
            .path()
            .join(TASSADAR_COMPILED_KERNEL_SUITE_ENVIRONMENT_BUNDLE_FILE)
            .exists());
        assert!(temp
            .path()
            .join(TASSADAR_COMPILED_KERNEL_SUITE_EXACTNESS_REPORT_FILE)
            .exists());
        assert!(temp
            .path()
            .join(TASSADAR_COMPILED_KERNEL_SUITE_COMPATIBILITY_REPORT_FILE)
            .exists());
        assert!(temp
            .path()
            .join(TASSADAR_COMPILED_KERNEL_SUITE_SCALING_REPORT_FILE)
            .exists());
        assert!(temp
            .path()
            .join(TASSADAR_COMPILED_KERNEL_SUITE_CLAIM_BOUNDARY_REPORT_FILE)
            .exists());
        assert!(temp
            .path()
            .join(TASSADAR_COMPILED_KERNEL_SUITE_ARTIFACT_FILE)
            .exists());
        assert!(temp.path().join(RUN_BUNDLE_FILE).exists());
        Ok(())
    }
}
