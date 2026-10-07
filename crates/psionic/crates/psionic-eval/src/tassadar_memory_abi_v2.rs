use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{TassadarMemoryAbiV2Publication, tassadar_memory_abi_v2_publication};
use psionic_runtime::{
    TassadarLinearMemoryExecutionError, TassadarLinearMemoryHaltReason,
    TassadarLinearMemoryProgram, TassadarLinearMemoryTraceFootprint,
    execute_tassadar_linear_memory_program, summarize_tassadar_linear_memory_trace_footprint,
    tassadar_seeded_linear_memory_copy_fill_program, tassadar_seeded_linear_memory_growth_program,
    tassadar_seeded_linear_memory_memcpy_program,
    tassadar_seeded_linear_memory_sign_extension_program,
    tassadar_seeded_linear_memory_width_parity_program,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_MEMORY_ABI_V2_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_MEMORY_ABI_V2_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_memory_abi_v2_report.json";

/// Repo-facing benchmark family for one memory ABI v2 case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMemoryAbiV2CaseFamily {
    /// Width parity and truncation behavior.
    LoadStoreWidthParity,
    /// Signed-vs-unsigned narrow loads.
    SignExtension,
    /// `memory.size` / `memory.grow` behavior.
    MemorySizeAndGrow,
    /// Dense memcpy-style trace regression coverage.
    MemcpyTraceRegression,
    /// Bulk-memory copy/fill exactness coverage.
    CopyFillExactness,
}

/// One repo-facing case report for the memory ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemoryAbiV2CaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Case family.
    pub family: TassadarMemoryAbiV2CaseFamily,
    /// Stable program identifier.
    pub program_id: String,
    /// Expected outputs for the seeded case.
    pub expected_outputs: Vec<i32>,
    /// Observed outputs from the runtime-owned execution.
    pub observed_outputs: Vec<i32>,
    /// Final halt reason.
    pub halt_reason: TassadarLinearMemoryHaltReason,
    /// Final memory digest for machine-legible comparison.
    pub final_memory_digest: String,
    /// Delta-vs-full trace footprint summary.
    pub trace_footprint: TassadarLinearMemoryTraceFootprint,
}

/// Committed report over the public memory ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMemoryAbiV2Report {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Model-facing publication for the lane.
    pub publication: TassadarMemoryAbiV2Publication,
    /// Ordered seeded-case results.
    pub cases: Vec<TassadarMemoryAbiV2CaseReport>,
    /// Explicit claim boundary for the current lane.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarMemoryAbiV2Report {
    fn new(cases: Vec<TassadarMemoryAbiV2CaseReport>) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_MEMORY_ABI_V2_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.memory_abi_v2.report.v1"),
            publication: tassadar_memory_abi_v2_publication(),
            cases,
            claim_boundary: String::from(
                "this report proves one public byte-addressed linear-memory lane with exact i8/i16/i32 width behavior, sign extension, memory.size, memory.grow, memory.copy, and memory.fill support, plus delta-oriented trace publication on bounded straight-line programs; it does not claim structured control flow, call frames, indirect calls, imports, globals, or full Wasm module closure",
            ),
            report_digest: String::new(),
        };
        report.report_digest = stable_digest(b"psionic_tassadar_memory_abi_v2_report|", &report);
        report
    }
}

/// Report build failures for the memory ABI v2 lane.
#[derive(Debug, Error)]
pub enum TassadarMemoryAbiV2ReportError {
    /// Runtime execution failed for one seeded case.
    #[error(transparent)]
    Runtime(#[from] TassadarLinearMemoryExecutionError),
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir {
        /// Directory path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to write the committed report.
    #[error("failed to write memory ABI v2 report `{path}`: {error}")]
    Write {
        /// Report path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to read a committed report.
    #[error("failed to read committed memory ABI v2 report `{path}`: {error}")]
    Read {
        /// Report path.
        path: String,
        /// OS error.
        error: std::io::Error,
    },
    /// Failed to decode a committed report.
    #[error("failed to decode committed memory ABI v2 report `{path}`: {error}")]
    Decode {
        /// Report path.
        path: String,
        /// JSON error.
        error: serde_json::Error,
    },
}

/// Builds the committed report for the public memory ABI v2 lane.
pub fn build_tassadar_memory_abi_v2_report()
-> Result<TassadarMemoryAbiV2Report, TassadarMemoryAbiV2ReportError> {
    let cases = vec![
        build_case(
            "width_parity",
            TassadarMemoryAbiV2CaseFamily::LoadStoreWidthParity,
            tassadar_seeded_linear_memory_width_parity_program(),
            vec![120, 22_136, 305_419_896, 255],
        )?,
        build_case(
            "sign_extension",
            TassadarMemoryAbiV2CaseFamily::SignExtension,
            tassadar_seeded_linear_memory_sign_extension_program(),
            vec![-128, 128, -128, 65_408],
        )?,
        build_case(
            "memory_size_and_grow",
            TassadarMemoryAbiV2CaseFamily::MemorySizeAndGrow,
            tassadar_seeded_linear_memory_growth_program(),
            vec![1, 1, 2, -1, 2],
        )?,
        build_case(
            "memcpy_trace_regression",
            TassadarMemoryAbiV2CaseFamily::MemcpyTraceRegression,
            tassadar_seeded_linear_memory_memcpy_program(64),
            vec![190],
        )?,
        build_case(
            "copy_fill_exactness",
            TassadarMemoryAbiV2CaseFamily::CopyFillExactness,
            tassadar_seeded_linear_memory_copy_fill_program(),
            vec![67_305_985, 134_678_021, 2_139_062_143],
        )?,
    ];
    Ok(TassadarMemoryAbiV2Report::new(cases))
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_memory_abi_v2_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MEMORY_ABI_V2_REPORT_REF)
}

/// Writes the committed report for the public memory ABI v2 lane.
pub fn write_tassadar_memory_abi_v2_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarMemoryAbiV2Report, TassadarMemoryAbiV2ReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| TassadarMemoryAbiV2ReportError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let report = build_tassadar_memory_abi_v2_report()?;
    let json = serde_json::to_string_pretty(&report)
        .expect("memory ABI v2 report serialization should succeed");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarMemoryAbiV2ReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_case(
    case_id: &str,
    family: TassadarMemoryAbiV2CaseFamily,
    program: TassadarLinearMemoryProgram,
    expected_outputs: Vec<i32>,
) -> Result<TassadarMemoryAbiV2CaseReport, TassadarMemoryAbiV2ReportError> {
    let execution = execute_tassadar_linear_memory_program(&program)?;
    let trace_footprint = summarize_tassadar_linear_memory_trace_footprint(&execution)?;
    Ok(TassadarMemoryAbiV2CaseReport {
        case_id: String::from(case_id),
        family,
        program_id: program.program_id,
        expected_outputs,
        observed_outputs: execution.outputs.clone(),
        halt_reason: execution.halt_reason,
        final_memory_digest: stable_digest(
            b"psionic_tassadar_linear_memory_final_memory|",
            &execution.final_memory,
        ),
        trace_footprint,
    })
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
        TASSADAR_MEMORY_ABI_V2_REPORT_REF, TassadarMemoryAbiV2CaseFamily,
        TassadarMemoryAbiV2Report, build_tassadar_memory_abi_v2_report, repo_root,
        write_tassadar_memory_abi_v2_report,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn memory_abi_v2_report_captures_seeded_truth() {
        let report = build_tassadar_memory_abi_v2_report().expect("report");
        assert_eq!(report.cases.len(), 5);
        let memcpy = report
            .cases
            .iter()
            .find(|case| case.family == TassadarMemoryAbiV2CaseFamily::MemcpyTraceRegression)
            .expect("memcpy case");
        assert_eq!(memcpy.expected_outputs, memcpy.observed_outputs);
        assert!(
            memcpy.trace_footprint.delta_trace_bytes
                < memcpy.trace_footprint.equivalent_full_snapshot_trace_bytes
        );
        let copy_fill = report
            .cases
            .iter()
            .find(|case| case.family == TassadarMemoryAbiV2CaseFamily::CopyFillExactness)
            .expect("copy/fill case");
        assert_eq!(copy_fill.expected_outputs, copy_fill.observed_outputs);
    }

    #[test]
    fn memory_abi_v2_report_matches_committed_truth() {
        let generated = build_tassadar_memory_abi_v2_report().expect("report");
        let committed: TassadarMemoryAbiV2Report =
            read_repo_json(TASSADAR_MEMORY_ABI_V2_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_memory_abi_v2_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory.path().join("tassadar_memory_abi_v2_report.json");
        let written = write_tassadar_memory_abi_v2_report(&output_path).expect("write report");
        let persisted: TassadarMemoryAbiV2Report =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
