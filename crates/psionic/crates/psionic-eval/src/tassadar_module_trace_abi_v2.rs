use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TassadarModuleTraceAbiV2Publication, tassadar_module_trace_abi_v2_publication,
};
use psionic_runtime::{
    TassadarModuleExecutionError, TassadarModuleExecutionHaltReason,
    TassadarModuleExecutionProgram, TassadarModuleTraceReplayError, TassadarModuleTraceV1Artifact,
    build_tassadar_module_trace_lineage_compatibility_receipt,
    build_tassadar_module_trace_v2_artifact, execute_tassadar_module_execution_program,
    replay_tassadar_module_trace_v2_artifact, summarize_tassadar_module_trace_footprint,
    tassadar_seeded_module_call_indirect_program,
    tassadar_seeded_module_deterministic_import_program,
    tassadar_seeded_module_global_state_program,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const TASSADAR_MODULE_TRACE_ABI_V2_REPORT_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_MODULE_TRACE_ABI_V2_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_trace_abi_v2_report.json";

/// Repo-facing family for one module-trace ABI v2 case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarModuleTraceAbiV2CaseFamily {
    /// Mutable-global exactness with delta publication.
    GlobalStateParity,
    /// Frame-aware indirect-call replay.
    CallIndirectDispatch,
    /// Deterministic host-import replay.
    DeterministicImportStub,
}

/// One repo-facing module-trace ABI v2 case report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceAbiV2CaseReport {
    /// Stable case identifier.
    pub case_id: String,
    /// Case family.
    pub family: TassadarModuleTraceAbiV2CaseFamily,
    /// Stable program identifier.
    pub program_id: String,
    /// Optional returned value from the execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub returned_value: Option<i32>,
    /// Terminal halt reason.
    pub halt_reason: TassadarModuleExecutionHaltReason,
    /// Stable execution digest shared across v1 and v2.
    pub execution_digest: String,
    /// Stable digest over the final globals.
    pub final_globals_digest: String,
    /// Maximum frame depth observed in the legacy execution.
    pub max_frame_depth: usize,
    /// Serialized byte count for the v1 step stream.
    pub v1_step_stream_bytes: u64,
    /// Serialized byte count for the v2 step stream.
    pub v2_step_stream_bytes: u64,
    /// Saved bytes between v1 and v2.
    pub step_stream_byte_savings: u64,
    /// Savings in basis points against v1.
    pub step_stream_reduction_bps: u32,
    /// Whether v2 replay rebuilt the exact legacy execution.
    pub replay_exact: bool,
    /// Shared lineage compatibility digest.
    pub lineage_compatibility_digest: String,
}

/// Committed report over the public module-trace ABI v2 lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarModuleTraceAbiV2Report {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Public publication for the lane.
    pub publication: TassadarModuleTraceAbiV2Publication,
    /// Ordered seeded-case results.
    pub cases: Vec<TassadarModuleTraceAbiV2CaseReport>,
    /// Explicit claim boundary for the current lane.
    pub claim_boundary: String,
    /// Stable digest over the report.
    pub report_digest: String,
}

impl TassadarModuleTraceAbiV2Report {
    fn new(cases: Vec<TassadarModuleTraceAbiV2CaseReport>) -> Self {
        let mut report = Self {
            schema_version: TASSADAR_MODULE_TRACE_ABI_V2_REPORT_SCHEMA_VERSION,
            report_id: String::from("tassadar.module_trace_abi_v2.report.v1"),
            publication: tassadar_module_trace_abi_v2_publication(),
            cases,
            claim_boundary: String::from(
                "this report proves a frame-aware delta-oriented trace ABI for the current bounded module-execution lane, with explicit v1/v2 versioning, deterministic replay back into the legacy snapshot-heavy trace, and shared final-state lineage truth; it does not claim arbitrary Wasm memory closure or successful traces for unsupported host imports",
            ),
            report_digest: String::new(),
        };
        report.report_digest =
            stable_digest(b"psionic_tassadar_module_trace_abi_v2_report|", &report);
        report
    }
}

/// Report build failures for the module-trace ABI v2 lane.
#[derive(Debug, Error)]
pub enum TassadarModuleTraceAbiV2ReportError {
    /// Runtime execution failed for one seeded case.
    #[error(transparent)]
    Runtime(#[from] TassadarModuleExecutionError),
    /// Step-stream footprint serialization failed.
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    /// V2 replay drifted from the legacy execution.
    #[error(transparent)]
    Replay(#[from] TassadarModuleTraceReplayError),
    /// Failed to create an output directory.
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    /// Failed to write the committed report.
    #[error("failed to write module trace ABI v2 report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    /// Failed to read a committed report.
    #[error("failed to read committed module trace ABI v2 report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    /// Failed to decode a committed report.
    #[error("failed to decode committed module trace ABI v2 report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

/// Builds the committed report for the public module-trace ABI v2 lane.
pub fn build_tassadar_module_trace_abi_v2_report()
-> Result<TassadarModuleTraceAbiV2Report, TassadarModuleTraceAbiV2ReportError> {
    Ok(TassadarModuleTraceAbiV2Report::new(vec![
        build_case(
            "global_state_parity",
            TassadarModuleTraceAbiV2CaseFamily::GlobalStateParity,
            tassadar_seeded_module_global_state_program(),
        )?,
        build_case(
            "call_indirect_dispatch",
            TassadarModuleTraceAbiV2CaseFamily::CallIndirectDispatch,
            tassadar_seeded_module_call_indirect_program(),
        )?,
        build_case(
            "deterministic_import_stub",
            TassadarModuleTraceAbiV2CaseFamily::DeterministicImportStub,
            tassadar_seeded_module_deterministic_import_program(),
        )?,
    ]))
}

/// Returns the canonical absolute path for the committed report.
pub fn tassadar_module_trace_abi_v2_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MODULE_TRACE_ABI_V2_REPORT_REF)
}

/// Writes the committed report for the public module-trace ABI v2 lane.
pub fn write_tassadar_module_trace_abi_v2_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarModuleTraceAbiV2Report, TassadarModuleTraceAbiV2ReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarModuleTraceAbiV2ReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_module_trace_abi_v2_report()?;
    let json = serde_json::to_string_pretty(&report)
        .expect("module trace ABI v2 report serialization should succeed");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarModuleTraceAbiV2ReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_case(
    case_id: &str,
    family: TassadarModuleTraceAbiV2CaseFamily,
    program: TassadarModuleExecutionProgram,
) -> Result<TassadarModuleTraceAbiV2CaseReport, TassadarModuleTraceAbiV2ReportError> {
    let execution = execute_tassadar_module_execution_program(&program)?;
    let v1 = TassadarModuleTraceV1Artifact::from_execution(
        format!("tassadar://module_trace_v1/report/{case_id}"),
        &execution,
    );
    let v2 = build_tassadar_module_trace_v2_artifact(
        format!("tassadar://module_trace_v2/report/{case_id}"),
        &program,
        &execution,
    )?;
    let replayed = replay_tassadar_module_trace_v2_artifact(&program, &v2)?;
    let footprint = summarize_tassadar_module_trace_footprint(&v1, &v2)?;
    let lineage = build_tassadar_module_trace_lineage_compatibility_receipt(&v1, &v2);

    Ok(TassadarModuleTraceAbiV2CaseReport {
        case_id: String::from(case_id),
        family,
        program_id: program.program_id,
        returned_value: execution.returned_value,
        halt_reason: execution.halt_reason,
        execution_digest: execution.execution_digest(),
        final_globals_digest: stable_digest(
            b"psionic_tassadar_module_trace_abi_v2_final_globals|",
            &execution.final_globals,
        ),
        max_frame_depth: execution
            .steps
            .iter()
            .map(|step| step.frame_depth_after)
            .max()
            .unwrap_or_default(),
        v1_step_stream_bytes: footprint.v1_step_stream_bytes,
        v2_step_stream_bytes: footprint.v2_step_stream_bytes,
        step_stream_byte_savings: footprint.step_stream_byte_savings,
        step_stream_reduction_bps: footprint.step_stream_reduction_bps,
        replay_exact: replayed == execution,
        lineage_compatibility_digest: lineage.compatibility_digest,
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
        TASSADAR_MODULE_TRACE_ABI_V2_REPORT_REF, TassadarModuleTraceAbiV2CaseFamily,
        TassadarModuleTraceAbiV2Report, build_tassadar_module_trace_abi_v2_report, repo_root,
        write_tassadar_module_trace_abi_v2_report,
    };

    fn read_repo_json<T: serde::de::DeserializeOwned>(
        repo_relative_path: &str,
    ) -> Result<T, Box<dyn std::error::Error>> {
        let path = repo_root().join(repo_relative_path);
        let bytes = std::fs::read(path)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[test]
    fn module_trace_abi_v2_report_captures_replay_and_reduction_truth() {
        let report = build_tassadar_module_trace_abi_v2_report().expect("report");
        assert_eq!(report.cases.len(), 3);
        assert!(report.cases.iter().all(|case| case.replay_exact));
        let call_indirect = report
            .cases
            .iter()
            .find(|case| case.family == TassadarModuleTraceAbiV2CaseFamily::CallIndirectDispatch)
            .expect("call-indirect case");
        assert!(call_indirect.step_stream_reduction_bps > 0);
        assert!(call_indirect.v2_step_stream_bytes < call_indirect.v1_step_stream_bytes);
    }

    #[test]
    fn module_trace_abi_v2_report_matches_committed_truth() {
        let generated = build_tassadar_module_trace_abi_v2_report().expect("report");
        let committed: TassadarModuleTraceAbiV2Report =
            read_repo_json(TASSADAR_MODULE_TRACE_ABI_V2_REPORT_REF).expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_module_trace_abi_v2_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_module_trace_abi_v2_report.json");
        let written =
            write_tassadar_module_trace_abi_v2_report(&output_path).expect("write report");
        let persisted: TassadarModuleTraceAbiV2Report =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
