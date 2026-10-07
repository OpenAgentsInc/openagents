use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_data::TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF;
use psionic_runtime::{
    build_tassadar_minimal_universal_substrate_runtime_report,
    TassadarMinimalUniversalSubstrateRuntimeReport,
    TassadarMinimalUniversalSubstrateRuntimeReportError,
    TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_RUNTIME_REPORT_REF,
};

use crate::{
    build_tassadar_universality_witness_suite_report, TassadarUniversalityWitnessSuiteReport,
    TassadarUniversalityWitnessSuiteReportError,
};

pub const TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_minimal_universal_substrate_acceptance_gate_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarMinimalUniversalSubstrateAcceptanceStatus {
    Green,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMinimalUniversalSubstrateAcceptanceRequirementRow {
    pub requirement_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarMinimalUniversalSubstrateAcceptanceGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_report_ref: String,
    pub witness_suite_report_ref: String,
    pub runtime_report: TassadarMinimalUniversalSubstrateRuntimeReport,
    pub witness_suite_report: TassadarUniversalityWitnessSuiteReport,
    pub requirement_rows: Vec<TassadarMinimalUniversalSubstrateAcceptanceRequirementRow>,
    pub green_requirement_ids: Vec<String>,
    pub failed_requirement_ids: Vec<String>,
    pub acceptance_status: TassadarMinimalUniversalSubstrateAcceptanceStatus,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarMinimalUniversalSubstrateAcceptanceGateReportError {
    #[error(transparent)]
    Runtime(#[from] TassadarMinimalUniversalSubstrateRuntimeReportError),
    #[error(transparent)]
    WitnessSuite(#[from] TassadarUniversalityWitnessSuiteReportError),
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

pub fn build_tassadar_minimal_universal_substrate_acceptance_gate_report() -> Result<
    TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    TassadarMinimalUniversalSubstrateAcceptanceGateReportError,
> {
    let runtime_report = build_tassadar_minimal_universal_substrate_runtime_report()?;
    let witness_suite_report = build_tassadar_universality_witness_suite_report()?;
    Ok(build_gate_report_from_inputs(
        runtime_report,
        witness_suite_report,
    ))
}

fn build_gate_report_from_inputs(
    runtime_report: TassadarMinimalUniversalSubstrateRuntimeReport,
    witness_suite_report: TassadarUniversalityWitnessSuiteReport,
) -> TassadarMinimalUniversalSubstrateAcceptanceGateReport {
    let requirement_rows = vec![
        requirement_row(
            "conditional_control_exact",
            runtime_report
                .green_requirement_ids
                .iter()
                .any(|id| id == "conditional_control_exact"),
            vec![String::from(
                "fixtures/tassadar/reports/tassadar_structured_control_report.json",
            )],
            "conditional control must stay exact before the minimal universal-substrate gate can turn green",
        ),
        requirement_row(
            "mutable_memory_growth",
            runtime_report
                .green_requirement_ids
                .iter()
                .any(|id| id == "mutable_memory_growth"),
            vec![String::from(
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
            )],
            "mutable memory and explicit growth must stay green before the minimal universal-substrate gate can turn green",
        ),
        requirement_row(
            "spill_tape_extension",
            runtime_report
                .green_requirement_ids
                .iter()
                .any(|id| id == "spill_tape_extension"),
            vec![String::from(
                "fixtures/tassadar/reports/tassadar_spill_tape_store_report.json",
            )],
            "spill and tape extension must stay explicit before the minimal universal-substrate gate can turn green",
        ),
        requirement_row(
            "persistent_continuation_resume",
            runtime_report
                .green_requirement_ids
                .iter()
                .any(|id| id == "persistent_continuation_resume"),
            vec![String::from(
                "fixtures/tassadar/reports/tassadar_execution_checkpoint_report.json",
            )],
            "persistent continuation and resume must stay explicit before the minimal universal-substrate gate can turn green",
        ),
        requirement_row(
            "machine_step_replay",
            runtime_report
                .green_requirement_ids
                .iter()
                .any(|id| id == "machine_step_replay"),
            vec![String::from(
                "fixtures/tassadar/runs/tassadar_universal_machine_simulation_v1/tassadar_universal_machine_simulation_bundle.json",
            )],
            "machine-step replay must stay explicit before the minimal universal-substrate gate can turn green",
        ),
        requirement_row(
            "universality_witness_suite",
            witness_suite_report.overall_green,
            vec![String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF)],
            "the dedicated witness suite must stay green before the minimal universal-substrate gate can turn green",
        ),
        requirement_row(
            "portability_envelope_declared",
            witness_suite_report
                .family_rows
                .iter()
                .all(|row| !row.runtime_envelope.trim().is_empty()),
            vec![String::from(
                TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_RUNTIME_REPORT_REF,
            )],
            "every witness row must carry an explicit runtime envelope before the minimal universal-substrate gate can turn green",
        ),
        requirement_row(
            "refusal_truth_explicit",
            witness_suite_report.refusal_boundary_count >= 2
                && witness_suite_report
                    .family_rows
                    .iter()
                    .filter(|row| row.expected_status == psionic_data::TassadarUniversalityWitnessExpectation::RefusalBoundary)
                    .all(|row| row.refusal_boundary_held),
            vec![String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF)],
            "refusal truth must stay explicit for out-of-profile behavior before the minimal universal-substrate gate can turn green",
        ),
    ];
    let green_requirement_ids = requirement_rows
        .iter()
        .filter(|row| row.satisfied)
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let failed_requirement_ids = requirement_rows
        .iter()
        .filter(|row| !row.satisfied)
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let acceptance_status = if failed_requirement_ids.is_empty() {
        TassadarMinimalUniversalSubstrateAcceptanceStatus::Green
    } else {
        TassadarMinimalUniversalSubstrateAcceptanceStatus::Failed
    };
    let overall_green =
        acceptance_status == TassadarMinimalUniversalSubstrateAcceptanceStatus::Green;
    let mut report = TassadarMinimalUniversalSubstrateAcceptanceGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.minimal_universal_substrate.acceptance_gate.report.v1"),
        runtime_report_ref: String::from(TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_RUNTIME_REPORT_REF),
        witness_suite_report_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
        runtime_report,
        witness_suite_report,
        requirement_rows,
        green_requirement_ids,
        failed_requirement_ids,
        acceptance_status,
        overall_green,
        claim_boundary: String::from(
            "this gate is the single machine-readable acceptance artifact for the minimal universal substrate. It turns green only when runtime prerequisites, witness coverage, portability envelopes, and refusal truth are all explicit. It does not by itself widen into theory/operator/served verdict splitting, served universality posture, arbitrary Wasm, or final Turing-complete closeout.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Minimal universal-substrate acceptance gate keeps green_requirement_ids={}, failed_requirement_ids={}, acceptance_status={:?}.",
        report.green_requirement_ids.len(),
        report.failed_requirement_ids.len(),
        report.acceptance_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_minimal_universal_substrate_acceptance_gate_report|",
        &report,
    );
    report
}

fn requirement_row(
    requirement_id: &str,
    satisfied: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarMinimalUniversalSubstrateAcceptanceRequirementRow {
    TassadarMinimalUniversalSubstrateAcceptanceRequirementRow {
        requirement_id: String::from(requirement_id),
        satisfied,
        source_refs,
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_minimal_universal_substrate_acceptance_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF)
}

pub fn write_tassadar_minimal_universal_substrate_acceptance_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    TassadarMinimalUniversalSubstrateAcceptanceGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarMinimalUniversalSubstrateAcceptanceGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_minimal_universal_substrate_acceptance_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarMinimalUniversalSubstrateAcceptanceGateReportError::Write {
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
) -> Result<T, TassadarMinimalUniversalSubstrateAcceptanceGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarMinimalUniversalSubstrateAcceptanceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarMinimalUniversalSubstrateAcceptanceGateReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_gate_report_from_inputs,
        build_tassadar_minimal_universal_substrate_acceptance_gate_report, read_json,
        tassadar_minimal_universal_substrate_acceptance_gate_report_path,
        TassadarMinimalUniversalSubstrateAcceptanceGateReport,
        TassadarMinimalUniversalSubstrateAcceptanceStatus,
        TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
    };
    use crate::build_tassadar_universality_witness_suite_report;
    use psionic_runtime::build_tassadar_minimal_universal_substrate_runtime_report;

    #[test]
    fn minimal_universal_substrate_acceptance_gate_turns_green_when_all_prereqs_hold() {
        let report =
            build_tassadar_minimal_universal_substrate_acceptance_gate_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(
            report.acceptance_status,
            TassadarMinimalUniversalSubstrateAcceptanceStatus::Green
        );
        assert_eq!(report.green_requirement_ids.len(), 8);
        assert!(report.failed_requirement_ids.is_empty());
    }

    #[test]
    fn minimal_universal_substrate_acceptance_gate_fails_when_witness_suite_is_mutated_red() {
        let runtime_report =
            build_tassadar_minimal_universal_substrate_runtime_report().expect("runtime report");
        let mut witness_suite_report =
            build_tassadar_universality_witness_suite_report().expect("witness suite");
        witness_suite_report.overall_green = false;
        if let Some(row) = witness_suite_report.family_rows.iter_mut().find(|row| {
            row.witness_family == psionic_data::TassadarUniversalityWitnessFamily::RegisterMachine
        }) {
            row.satisfied = false;
        }

        let report = build_gate_report_from_inputs(runtime_report, witness_suite_report);

        assert!(!report.overall_green);
        assert_eq!(
            report.acceptance_status,
            TassadarMinimalUniversalSubstrateAcceptanceStatus::Failed
        );
        assert!(report
            .failed_requirement_ids
            .contains(&String::from("universality_witness_suite")));
    }

    #[test]
    fn minimal_universal_substrate_acceptance_gate_matches_committed_truth() {
        let generated =
            build_tassadar_minimal_universal_substrate_acceptance_gate_report().expect("report");
        let committed: TassadarMinimalUniversalSubstrateAcceptanceGateReport =
            read_json(tassadar_minimal_universal_substrate_acceptance_gate_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_minimal_universal_substrate_acceptance_gate_report.json"
        );
    }
}
