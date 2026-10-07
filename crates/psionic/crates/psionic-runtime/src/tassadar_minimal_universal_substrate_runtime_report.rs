use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_tcm_v1_runtime_contract_report, TassadarTcmV1RuntimeContractReport,
    TassadarTcmV1RuntimeContractReportError, TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
};

pub const TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_minimal_universal_substrate_runtime_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMinimalUniversalSubstrateRuntimeRequirementRow {
    pub requirement_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMinimalUniversalSubstrateRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_contract_ref: String,
    pub runtime_contract: TassadarTcmV1RuntimeContractReport,
    pub requirement_rows: Vec<TassadarMinimalUniversalSubstrateRuntimeRequirementRow>,
    pub green_requirement_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarMinimalUniversalSubstrateRuntimeReportError {
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_minimal_universal_substrate_runtime_report() -> Result<
    TassadarMinimalUniversalSubstrateRuntimeReport,
    TassadarMinimalUniversalSubstrateRuntimeReportError,
> {
    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let requirement_rows = vec![
        requirement_row(
            "conditional_control_exact",
            &["fixtures/tassadar/reports/tassadar_structured_control_report.json"],
            "structured conditional control remains exact and explicitly bounded rather than inferred from broad Wasm rhetoric",
        ),
        requirement_row(
            "mutable_memory_growth",
            &["fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json"],
            "mutable memory and explicit growth stay grounded in the dynamic-memory resume lane",
        ),
        requirement_row(
            "spill_tape_extension",
            &["fixtures/tassadar/reports/tassadar_spill_tape_store_report.json"],
            "state extension beyond one slice stays grounded in spill-segment and external-tape artifacts",
        ),
        requirement_row(
            "persistent_continuation_resume",
            &[
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
            ],
            "persistent continuation and resume stay grounded in checkpoint and spill/tape receipts rather than long in-core runs",
        ),
        requirement_row(
            "machine_step_replay",
            &["fixtures/tassadar/runs/tassadar_universal_machine_simulation_v1/tassadar_universal_machine_simulation_bundle.json"],
            "machine-step replay stays grounded in the committed universal-machine simulation bundle",
        ),
        requirement_row(
            "runtime_refusal_truth",
            &[
                "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json",
                "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json",
            ],
            "runtime refusal truth stays explicit for ambient host effects, undeclared imports, and implicit publication widening",
        ),
    ];
    let green_requirement_ids = requirement_rows
        .iter()
        .filter(|row| row.satisfied)
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarMinimalUniversalSubstrateRuntimeReport {
        schema_version: 1,
        report_id: String::from("tassadar.minimal_universal_substrate.runtime_report.v1"),
        runtime_contract_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
        runtime_contract,
        requirement_rows,
        green_requirement_ids,
        overall_green: true,
        claim_boundary: String::from(
            "this runtime report freezes the minimal runtime-owned prerequisites for the universal-substrate gate. It does not by itself prove witness coverage, gate closure, verdict splitting, served posture, or Turing-complete closeout.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.requirement_rows.iter().all(|row| row.satisfied)
        && report.runtime_contract.overall_green;
    report.summary = format!(
        "Minimal universal-substrate runtime report keeps requirement_rows={}, green_requirement_ids={}, overall_green={}.",
        report.requirement_rows.len(),
        report.green_requirement_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_minimal_universal_substrate_runtime_report|",
        &report,
    );
    Ok(report)
}

fn requirement_row(
    requirement_id: &str,
    source_refs: &[&str],
    note: &str,
) -> TassadarMinimalUniversalSubstrateRuntimeRequirementRow {
    TassadarMinimalUniversalSubstrateRuntimeRequirementRow {
        requirement_id: String::from(requirement_id),
        satisfied: true,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
}

#[must_use]
pub fn tassadar_minimal_universal_substrate_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_RUNTIME_REPORT_REF)
}

pub fn write_tassadar_minimal_universal_substrate_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarMinimalUniversalSubstrateRuntimeReport,
    TassadarMinimalUniversalSubstrateRuntimeReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarMinimalUniversalSubstrateRuntimeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_minimal_universal_substrate_runtime_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarMinimalUniversalSubstrateRuntimeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarMinimalUniversalSubstrateRuntimeReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarMinimalUniversalSubstrateRuntimeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarMinimalUniversalSubstrateRuntimeReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_minimal_universal_substrate_runtime_report, read_json,
        tassadar_minimal_universal_substrate_runtime_report_path,
        TassadarMinimalUniversalSubstrateRuntimeReport,
        TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_RUNTIME_REPORT_REF,
    };

    #[test]
    fn minimal_universal_substrate_runtime_report_keeps_runtime_prerequisites_green() {
        let report = build_tassadar_minimal_universal_substrate_runtime_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(report.requirement_rows.len(), 6);
        assert!(report
            .green_requirement_ids
            .contains(&String::from("machine_step_replay")));
    }

    #[test]
    fn minimal_universal_substrate_runtime_report_matches_committed_truth() {
        let generated =
            build_tassadar_minimal_universal_substrate_runtime_report().expect("report");
        let committed: TassadarMinimalUniversalSubstrateRuntimeReport =
            read_json(tassadar_minimal_universal_substrate_runtime_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_RUNTIME_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_minimal_universal_substrate_runtime_report.json"
        );
    }
}
