use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    TassadarCompileRefusal, TassadarCompilerToolchainIdentity, TassadarRustToWasmCompileConfig,
    TassadarRustToWasmCompileOutcome, TassadarRustToWasmCompileReceipt, TassadarWasmBinarySummary,
    compile_tassadar_rust_source_to_wasm_receipt, tassadar_heap_sum_rust_source_path,
    tassadar_heap_sum_rust_wasm_binary_path, tassadar_hungarian_10x10_rust_source_path,
    tassadar_hungarian_10x10_rust_wasm_binary_path, tassadar_long_loop_rust_source_path,
    tassadar_long_loop_rust_wasm_binary_path, tassadar_memory_lookup_rust_source_path,
    tassadar_memory_lookup_rust_wasm_binary_path, tassadar_micro_wasm_rust_source_path,
    tassadar_micro_wasm_rust_wasm_binary_path, tassadar_multi_export_rust_source_path,
    tassadar_multi_export_rust_wasm_binary_path, tassadar_param_abi_rust_source_path,
    tassadar_param_abi_rust_wasm_binary_path, tassadar_sudoku_9x9_rust_source_path,
    tassadar_sudoku_9x9_rust_wasm_binary_path,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_RUST_SOURCE_CANON_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_RUST_SOURCE_CANON_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_source_canon_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRustSourceCanonCaseStatus {
    Compiled,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustSourceCanonCase {
    pub case_id: String,
    pub workload_family_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub compile_config_digest: String,
    pub compile_pipeline_features: Vec<String>,
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    pub toolchain_digest: String,
    pub receipt_digest: String,
    pub status: TassadarRustSourceCanonCaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_summary: Option<TassadarWasmBinarySummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal_detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustSourceCanonReport {
    pub schema_version: u16,
    pub report_id: String,
    pub source_family_ids: Vec<String>,
    pub generated_from_refs: Vec<String>,
    pub cases: Vec<TassadarRustSourceCanonCase>,
    pub claim_boundary: String,
    pub report_digest: String,
}

impl TassadarRustSourceCanonReport {
    fn new(cases: Vec<TassadarRustSourceCanonCase>) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_RUST_SOURCE_CANON_SCHEMA_VERSION,
            report_id: String::from("tassadar.rust_source_canon.report.v1"),
            source_family_ids: cases
                .iter()
                .map(|case| case.workload_family_id.clone())
                .collect(),
            generated_from_refs: cases.iter().map(|case| case.source_ref.clone()).collect(),
            cases,
            claim_boundary: String::from(
                "this report freezes the Rust-only frontend canon for the Tassadar article-closure path. It proves explicit Rust-source, toolchain, compile-config, and Wasm-output lineage for the committed kernel, heap-input, long-loop, Hungarian, and Sudoku fixtures, and it excludes the older C-source receipt from the article-closure path. It does not by itself imply generic Rust frontend closure, bounded-lane lowering coverage, or arbitrary Wasm execution support.",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_rust_source_canon_report|", &report);
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarRustSourceCanonReportError {
    #[error("failed to read source fixture `{path}`: {error}")]
    ReadSource { path: String, error: std::io::Error },
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

pub fn build_tassadar_rust_source_canon_report()
-> Result<TassadarRustSourceCanonReport, TassadarRustSourceCanonReportError> {
    Ok(TassadarRustSourceCanonReport::new(vec![
        build_case(
            "multi_export_exact",
            "rust.multi_export_kernel",
            tassadar_multi_export_rust_source_path(),
            tassadar_multi_export_rust_wasm_binary_path(),
            TassadarRustToWasmCompileConfig::canonical_multi_export_kernel(),
        )?,
        build_case(
            "memory_lookup_exact",
            "rust.memory_lookup_kernel",
            tassadar_memory_lookup_rust_source_path(),
            tassadar_memory_lookup_rust_wasm_binary_path(),
            TassadarRustToWasmCompileConfig::canonical_memory_lookup_kernel(),
        )?,
        build_case(
            "param_abi_fixture",
            "rust.param_abi_kernel",
            tassadar_param_abi_rust_source_path(),
            tassadar_param_abi_rust_wasm_binary_path(),
            TassadarRustToWasmCompileConfig::canonical_param_abi_kernel(),
        )?,
        build_case(
            "micro_wasm_article",
            "rust.micro_wasm_article",
            tassadar_micro_wasm_rust_source_path(),
            tassadar_micro_wasm_rust_wasm_binary_path(),
            TassadarRustToWasmCompileConfig::canonical_micro_wasm_kernel(),
        )?,
        build_case(
            "heap_sum_article",
            "rust.heap_sum_article",
            tassadar_heap_sum_rust_source_path(),
            tassadar_heap_sum_rust_wasm_binary_path(),
            TassadarRustToWasmCompileConfig::canonical_heap_sum_kernel(),
        )?,
        build_case(
            "long_loop_article",
            "rust.long_loop_article",
            tassadar_long_loop_rust_source_path(),
            tassadar_long_loop_rust_wasm_binary_path(),
            TassadarRustToWasmCompileConfig::canonical_long_loop_kernel(),
        )?,
        build_case(
            "hungarian_10x10_article",
            "rust.hungarian_10x10_article",
            tassadar_hungarian_10x10_rust_source_path(),
            tassadar_hungarian_10x10_rust_wasm_binary_path(),
            TassadarRustToWasmCompileConfig::canonical_hungarian_10x10_article(),
        )?,
        build_case(
            "sudoku_9x9_article",
            "rust.sudoku_9x9_article",
            tassadar_sudoku_9x9_rust_source_path(),
            tassadar_sudoku_9x9_rust_wasm_binary_path(),
            TassadarRustToWasmCompileConfig::canonical_sudoku_9x9_article(),
        )?,
    ]))
}

pub fn tassadar_rust_source_canon_report_path() -> PathBuf {
    repo_root().join(TASSADAR_RUST_SOURCE_CANON_REPORT_REF)
}

pub fn write_tassadar_rust_source_canon_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarRustSourceCanonReport, TassadarRustSourceCanonReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarRustSourceCanonReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_rust_source_canon_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("Rust source canon report should serialize");
    fs::write(output_path, bytes).map_err(|error| TassadarRustSourceCanonReportError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

fn build_case(
    case_id: &str,
    workload_family_id: &str,
    source_path: PathBuf,
    output_wasm_path: PathBuf,
    compile_config: TassadarRustToWasmCompileConfig,
) -> Result<TassadarRustSourceCanonCase, TassadarRustSourceCanonReportError> {
    let source_bytes =
        fs::read(&source_path).map_err(|error| TassadarRustSourceCanonReportError::ReadSource {
            path: source_path.display().to_string(),
            error,
        })?;
    let receipt = compile_tassadar_rust_source_to_wasm_receipt(
        canonical_repo_relative_path(&source_path),
        &source_bytes,
        &output_wasm_path,
        &compile_config,
    );
    Ok(case_from_receipt(
        case_id,
        workload_family_id,
        &compile_config,
        &receipt,
    ))
}

fn case_from_receipt(
    case_id: &str,
    workload_family_id: &str,
    compile_config: &TassadarRustToWasmCompileConfig,
    receipt: &TassadarRustToWasmCompileReceipt,
) -> TassadarRustSourceCanonCase {
    let source_ref = receipt.source_identity.source_name.clone();
    let source_digest = receipt.source_identity.source_digest.clone();
    let toolchain_identity = receipt.toolchain_identity.clone();
    let toolchain_digest = stable_digest(
        b"psionic_tassadar_rust_source_canon_toolchain|",
        &toolchain_identity,
    );
    let (
        status,
        wasm_binary_ref,
        wasm_binary_digest,
        wasm_binary_summary,
        refusal_kind,
        refusal_detail,
    ) = match &receipt.outcome {
        TassadarRustToWasmCompileOutcome::Succeeded {
            wasm_binary_ref,
            wasm_binary_digest,
            wasm_binary_summary,
        } => (
            TassadarRustSourceCanonCaseStatus::Compiled,
            Some(wasm_binary_ref.clone()),
            Some(wasm_binary_digest.clone()),
            Some(wasm_binary_summary.clone()),
            None,
            None,
        ),
        TassadarRustToWasmCompileOutcome::Refused { refusal } => (
            TassadarRustSourceCanonCaseStatus::Refused,
            None,
            None,
            None,
            Some(refusal.kind_slug().to_string()),
            Some(refusal_detail(refusal)),
        ),
    };
    TassadarRustSourceCanonCase {
        case_id: String::from(case_id),
        workload_family_id: String::from(workload_family_id),
        source_ref,
        source_digest,
        compile_config_digest: compile_config.stable_digest(),
        compile_pipeline_features: compile_config.pipeline_features(),
        toolchain_identity,
        toolchain_digest,
        receipt_digest: receipt.receipt_digest.clone(),
        status,
        wasm_binary_ref,
        wasm_binary_digest,
        wasm_binary_summary,
        refusal_kind,
        refusal_detail,
    }
}

fn refusal_detail(refusal: &TassadarCompileRefusal) -> String {
    match refusal {
        TassadarCompileRefusal::ToolchainFailure { stderr_excerpt, .. } => stderr_excerpt.clone(),
        other => other.to_string(),
    }
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
mod tests {
    use super::{
        TASSADAR_RUST_SOURCE_CANON_REPORT_REF, TassadarRustSourceCanonCaseStatus,
        TassadarRustSourceCanonReport, build_tassadar_rust_source_canon_report, repo_root,
        write_tassadar_rust_source_canon_report,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn rust_source_canon_replaces_the_c_frontend_in_article_closure() {
        let report = build_tassadar_rust_source_canon_report().expect("report");
        assert_eq!(report.cases.len(), 8);
        assert!(
            report
                .generated_from_refs
                .iter()
                .all(|source_ref| source_ref.ends_with(".rs"))
        );
        assert!(
            report
                .cases
                .iter()
                .filter(|case| case.case_id.contains("article"))
                .all(|case| case.status == TassadarRustSourceCanonCaseStatus::Compiled)
        );
    }

    #[test]
    fn rust_source_canon_report_matches_committed_truth() -> Result<(), Box<dyn std::error::Error>>
    {
        let report = build_tassadar_rust_source_canon_report()?;
        let committed: TassadarRustSourceCanonReport =
            read_repo_json(TASSADAR_RUST_SOURCE_CANON_REPORT_REF)?;
        assert_eq!(report, committed);
        Ok(())
    }

    #[test]
    fn write_rust_source_canon_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let report_path = temp_dir
            .path()
            .join("tassadar_rust_source_canon_report.json");
        let report = write_tassadar_rust_source_canon_report(&report_path)?;
        let persisted: TassadarRustSourceCanonReport =
            serde_json::from_slice(&std::fs::read(&report_path)?)?;
        assert_eq!(report, persisted);
        Ok(())
    }
}
