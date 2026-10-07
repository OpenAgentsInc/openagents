use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TassadarArticleCpuReproducibilityMatrix, TassadarArticleRuntimeCloseoutError,
    TassadarCompilerToolchainIdentity,
    build_tassadar_article_cpu_reproducibility_matrix,
};

use crate::{
    TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF, TASSADAR_RUST_SOURCE_CANON_REPORT_REF,
    TassadarCompilePipelineMatrixCaseStatus, TassadarCompilePipelineMatrixReportError,
    TassadarRustSourceCanonCaseStatus,
    TassadarRustSourceCanonReportError, build_tassadar_compile_pipeline_matrix_report,
    build_tassadar_rust_source_canon_report,
};

pub const TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_cpu_reproducibility_report.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleCpuReproducibilityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub source_canon_report_ref: String,
    pub compile_pipeline_matrix_report_ref: String,
    pub matrix: TassadarArticleCpuReproducibilityMatrix,
    pub rust_toolchain_identity: TassadarCompilerToolchainIdentity,
    pub rust_toolchain_digest: String,
    pub rust_toolchain_case_ids: Vec<String>,
    pub rust_toolchain_case_count: u32,
    pub rust_toolchain_uniform: bool,
    pub optional_c_path_case_id: String,
    pub optional_c_path_status: TassadarCompilePipelineMatrixCaseStatus,
    pub optional_c_path_toolchain_identity: TassadarCompilerToolchainIdentity,
    pub optional_c_path_toolchain_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional_c_path_compile_refusal_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optional_c_path_compile_refusal_detail: Option<String>,
    pub optional_c_path_blocks_rust_only_claim: bool,
    pub supported_machine_class_ids: Vec<String>,
    pub unsupported_machine_class_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarArticleCpuReproducibilityReport {
    fn new(
        matrix: TassadarArticleCpuReproducibilityMatrix,
        rust_toolchain_identity: TassadarCompilerToolchainIdentity,
        rust_toolchain_case_ids: Vec<String>,
        rust_toolchain_uniform: bool,
        optional_c_path_case_id: String,
        optional_c_path_status: TassadarCompilePipelineMatrixCaseStatus,
        optional_c_path_toolchain_identity: TassadarCompilerToolchainIdentity,
        optional_c_path_compile_refusal_kind: Option<String>,
        optional_c_path_compile_refusal_detail: Option<String>,
    ) -> Self {
        let rust_toolchain_digest = stable_digest(
            b"psionic_tassadar_article_cpu_reproducibility_rust_toolchain|",
            &rust_toolchain_identity,
        );
        let optional_c_path_toolchain_digest = stable_digest(
            b"psionic_tassadar_article_cpu_reproducibility_c_toolchain|",
            &optional_c_path_toolchain_identity,
        );
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.article_cpu_reproducibility.report.v1"),
            source_canon_report_ref: String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
            compile_pipeline_matrix_report_ref: String::from(TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF),
            supported_machine_class_ids: matrix.supported_machine_class_ids.clone(),
            unsupported_machine_class_ids: matrix.unsupported_machine_class_ids.clone(),
            matrix,
            rust_toolchain_identity,
            rust_toolchain_digest,
            rust_toolchain_case_count: rust_toolchain_case_ids.len() as u32,
            rust_toolchain_case_ids,
            rust_toolchain_uniform,
            optional_c_path_case_id,
            optional_c_path_status,
            optional_c_path_toolchain_identity,
            optional_c_path_toolchain_digest,
            optional_c_path_compile_refusal_kind,
            optional_c_path_compile_refusal_detail,
            optional_c_path_blocks_rust_only_claim: false,
            claim_boundary: String::from(
                "this report joins the current-host CPU reproducibility matrix to the Rust-only source canon and the optional C-path compile boundary. It closes portability only for the declared x86_64/aarch64 CPU families on the Rust-only article path, keeps the optional C-path toolchain refusal explicit, and does not imply universal portability or non-CPU backend closure",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "Rust-only article CPU reproducibility now binds current_host=`{}` with green_current_host={}, supported_classes={}, optional_c_path_status=`{:?}`.",
            report.matrix.current_host_machine_class_id,
            report.matrix.current_host_measured_green,
            report.supported_machine_class_ids.len(),
            report.optional_c_path_status,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_article_cpu_reproducibility_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleCpuReproducibilityReportError {
    #[error(transparent)]
    Runtime(#[from] TassadarArticleRuntimeCloseoutError),
    #[error(transparent)]
    RustSourceCanon(#[from] TassadarRustSourceCanonReportError),
    #[error(transparent)]
    CompilePipelineMatrix(#[from] TassadarCompilePipelineMatrixReportError),
    #[error("missing compiled Rust source canon case")]
    MissingCompiledRustSourceCanonCase,
    #[error("missing compile-pipeline C path case")]
    MissingCompilePipelineCPathCase,
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read committed report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode committed report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_article_cpu_reproducibility_report()
-> Result<TassadarArticleCpuReproducibilityReport, TassadarArticleCpuReproducibilityReportError> {
    let matrix = build_tassadar_article_cpu_reproducibility_matrix()?;
    let source_report = build_tassadar_rust_source_canon_report()?;
    let compile_report = build_tassadar_compile_pipeline_matrix_report()?;

    let compiled_cases = source_report
        .cases
        .iter()
        .filter(|case| case.status == TassadarRustSourceCanonCaseStatus::Compiled)
        .collect::<Vec<_>>();
    let first_compiled_case = compiled_cases
        .first()
        .ok_or(TassadarArticleCpuReproducibilityReportError::MissingCompiledRustSourceCanonCase)?;
    let shared_pipeline_features = compiled_cases.iter().fold(
        first_compiled_case
            .toolchain_identity
            .pipeline_features
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        |shared, case| {
            let case_features = case
                .toolchain_identity
                .pipeline_features
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            shared
                .intersection(&case_features)
                .cloned()
                .collect::<BTreeSet<_>>()
        },
    );
    let rust_toolchain_identity = TassadarCompilerToolchainIdentity::new(
        first_compiled_case
            .toolchain_identity
            .compiler_family
            .clone(),
        first_compiled_case
            .toolchain_identity
            .compiler_version
            .clone(),
        first_compiled_case.toolchain_identity.target.clone(),
    )
    .with_pipeline_features(shared_pipeline_features.into_iter().collect());
    let rust_toolchain_case_ids = compiled_cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let rust_toolchain_uniform = compiled_cases
        .iter()
        .all(|case| {
            case.toolchain_identity.compiler_family == rust_toolchain_identity.compiler_family
                && case.toolchain_identity.compiler_version
                    == rust_toolchain_identity.compiler_version
                && case.toolchain_identity.target == rust_toolchain_identity.target
        });

    let c_case = compile_report
        .cases
        .iter()
        .find(|case| case.case_id == "c_missing_toolchain_refusal")
        .ok_or(TassadarArticleCpuReproducibilityReportError::MissingCompilePipelineCPathCase)?;

    Ok(TassadarArticleCpuReproducibilityReport::new(
        matrix,
        rust_toolchain_identity,
        rust_toolchain_case_ids,
        rust_toolchain_uniform,
        c_case.case_id.clone(),
        c_case.status,
        c_case.toolchain_identity.clone(),
        c_case.compile_refusal_kind.clone(),
        c_case.compile_refusal_detail.clone(),
    ))
}

pub fn tassadar_article_cpu_reproducibility_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF)
}

pub fn write_tassadar_article_cpu_reproducibility_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleCpuReproducibilityReport, TassadarArticleCpuReproducibilityReportError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleCpuReproducibilityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_cpu_reproducibility_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("article cpu reproducibility report serializes");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarArticleCpuReproducibilityReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleCpuReproducibilityReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleCpuReproducibilityReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleCpuReproducibilityReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
        TassadarArticleCpuReproducibilityReport, build_tassadar_article_cpu_reproducibility_report,
        read_repo_json, tassadar_article_cpu_reproducibility_report_path,
        write_tassadar_article_cpu_reproducibility_report,
    };

    #[test]
    fn article_cpu_reproducibility_report_names_rust_and_optional_c_toolchains() {
        let report = build_tassadar_article_cpu_reproducibility_report().expect("report");

        assert_eq!(report.rust_toolchain_case_count, 8);
        assert!(report.rust_toolchain_uniform);
        assert_eq!(report.rust_toolchain_identity.compiler_family, "rustc");
        assert_eq!(report.optional_c_path_case_id, "c_missing_toolchain_refusal");
        assert!(!report.optional_c_path_blocks_rust_only_claim);
    }

    #[test]
    fn article_cpu_reproducibility_report_matches_committed_truth() {
        let generated = build_tassadar_article_cpu_reproducibility_report().expect("report");
        let committed: TassadarArticleCpuReproducibilityReport = read_repo_json(
            TASSADAR_ARTICLE_CPU_REPRODUCIBILITY_REPORT_REF,
            "tassadar_article_cpu_reproducibility_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_cpu_reproducibility_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_cpu_reproducibility_report.json");
        let written = write_tassadar_article_cpu_reproducibility_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleCpuReproducibilityReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_cpu_reproducibility_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_cpu_reproducibility_report.json")
        );
    }
}
