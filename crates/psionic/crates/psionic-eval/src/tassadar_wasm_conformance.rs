use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_runtime::{
    TASSADAR_WASM_CONFORMANCE_GENERATOR_SEED, TASSADAR_WASM_REFERENCE_AUTHORITY_ID,
    TassadarModuleExecutionDifferentialResult, run_tassadar_module_execution_differential,
    tassadar_curated_wasm_conformance_cases, tassadar_generated_wasm_conformance_cases,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_WASM_CONFORMANCE_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_WASM_CONFORMANCE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json";

/// Committed report over the bounded module-execution Wasm conformance harness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarWasmConformanceReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable reference-authority identifier.
    pub reference_authority_id: String,
    /// Stable deterministic seed for the generated case slice.
    pub generator_seed: u64,
    /// Stable case-family identifiers covered by the report.
    pub case_family_ids: Vec<String>,
    /// Ordered curated and generated differential cases.
    pub cases: Vec<TassadarModuleExecutionDifferentialResult>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarWasmConformanceReport {
    fn new(cases: Vec<TassadarModuleExecutionDifferentialResult>) -> Self {
        let case_family_ids = cases
            .iter()
            .map(|case| case.family_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut report = Self {
            schema_version: TASSADAR_WASM_CONFORMANCE_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.wasm_conformance.report.v1"),
            reference_authority_id: String::from(TASSADAR_WASM_REFERENCE_AUTHORITY_ID),
            generator_seed: TASSADAR_WASM_CONFORMANCE_GENERATOR_SEED,
            case_family_ids,
            cases,
            claim_boundary: String::from(
                "this report differentially checks the current bounded module-execution lane against a real Wasm reference authority over curated and deterministically generated cases; exact success and trap parity are only claimed for the current supported i32 global/table/direct-call/call_indirect/start-instantiation subset, while unsupported host-import behavior remains an explicit runtime refusal boundary instead of arbitrary Wasm closure",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(b"psionic_tassadar_wasm_conformance_report|", &report);
        report
    }
}

/// Report build failures for the Wasm conformance harness.
#[derive(Debug, Error)]
pub enum TassadarWasmConformanceReportError {
    /// Runtime differential execution failed.
    #[error(transparent)]
    Runtime(#[from] psionic_runtime::TassadarWasmConformanceError),
    /// Failed to create the output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the report.
    #[error("failed to write Wasm conformance report `{path}`: {error}")]
    Write {
        /// File path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
}

/// Builds the committed Wasm conformance report for the bounded module lane.
pub fn build_tassadar_wasm_conformance_report()
-> Result<TassadarWasmConformanceReport, TassadarWasmConformanceReportError> {
    let mut cases = Vec::new();
    for case in tassadar_curated_wasm_conformance_cases() {
        cases.push(run_tassadar_module_execution_differential(&case)?);
    }
    for case in
        tassadar_generated_wasm_conformance_cases(TASSADAR_WASM_CONFORMANCE_GENERATOR_SEED, 4, 2)
    {
        cases.push(run_tassadar_module_execution_differential(&case)?);
    }
    Ok(TassadarWasmConformanceReport::new(cases))
}

/// Returns the canonical absolute path for the committed Wasm conformance report.
pub fn tassadar_wasm_conformance_report_path() -> PathBuf {
    repo_root().join(TASSADAR_WASM_CONFORMANCE_REPORT_REF)
}

/// Writes the committed Wasm conformance report.
pub fn write_tassadar_wasm_conformance_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarWasmConformanceReport, TassadarWasmConformanceReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarWasmConformanceReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_wasm_conformance_report()?;
    let bytes =
        serde_json::to_vec_pretty(&report).expect("Wasm conformance report should serialize");
    fs::write(output_path, bytes).map_err(|error| TassadarWasmConformanceReportError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use psionic_runtime::TassadarModuleDifferentialStatus;

    use super::{
        TASSADAR_WASM_CONFORMANCE_REPORT_REF, TassadarWasmConformanceReport,
        build_tassadar_wasm_conformance_report, tassadar_wasm_conformance_report_path,
        write_tassadar_wasm_conformance_report,
    };

    #[test]
    fn wasm_conformance_report_covers_curated_and_generated_cases() {
        let report =
            build_tassadar_wasm_conformance_report().expect("Wasm conformance report should build");
        assert_eq!(report.reference_authority_id, "wasmi.reference.v1");
        assert_eq!(report.cases.len(), 12);
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.family_id == "curated.global_state"
                    && case.status == TassadarModuleDifferentialStatus::ExactSuccess)
        );
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.family_id == "curated.instantiation"
                    && case.status == TassadarModuleDifferentialStatus::ExactSuccess)
        );
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.family_id == "curated.call_indirect_trap"
                    && case.status == TassadarModuleDifferentialStatus::ExactTrapParity)
        );
        assert!(
            report
                .cases
                .iter()
                .any(|case| case.family_id == "curated.unsupported_host_import"
                    && case.status == TassadarModuleDifferentialStatus::BoundaryRefusal)
        );
        assert!(
            report
                .cases
                .iter()
                .filter(|case| case.family_id.starts_with("generated."))
                .count()
                >= 6
        );
    }

    #[test]
    fn wasm_conformance_report_matches_committed_truth() {
        let report =
            build_tassadar_wasm_conformance_report().expect("Wasm conformance report should build");
        let path = tassadar_wasm_conformance_report_path();
        let bytes = std::fs::read(&path).expect("committed report should exist");
        let persisted: TassadarWasmConformanceReport =
            serde_json::from_slice(&bytes).expect("committed report should decode");
        assert_eq!(
            persisted,
            report,
            "run the example to refresh `{}`",
            path.display()
        );
    }

    #[test]
    fn write_wasm_conformance_report_persists_current_truth() {
        let output_path = std::env::temp_dir().join("tassadar_wasm_conformance_report.json");
        let report = write_tassadar_wasm_conformance_report(&output_path)
            .expect("Wasm conformance report should write");
        let bytes = std::fs::read(&output_path).expect("persisted report should exist");
        let persisted: TassadarWasmConformanceReport =
            serde_json::from_slice(&bytes).expect("persisted report should decode");
        assert_eq!(persisted, report);
        std::fs::remove_file(&output_path).expect("temp report should be removable");
        assert_eq!(
            PathBuf::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF)
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_wasm_conformance_report.json")
        );
    }
}
