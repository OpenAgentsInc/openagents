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
    TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF, TassadarTcmV1RuntimeContractReport,
    TassadarTcmV1RuntimeContractReportError, build_tassadar_tcm_v1_runtime_contract_report,
};

use crate::{
    TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF, TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
    TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    TassadarMinimalUniversalSubstrateAcceptanceGateReportError,
    TassadarUniversalMachineProofReport, TassadarUniversalMachineProofReportError,
    TassadarUniversalityVerdictSplitReport, TassadarUniversalityVerdictSplitReportError,
    TassadarUniversalityWitnessSuiteReport, TassadarUniversalityWitnessSuiteReportError,
    build_tassadar_minimal_universal_substrate_acceptance_gate_report,
    build_tassadar_universal_machine_proof_report,
    build_tassadar_universality_verdict_split_report,
    build_tassadar_universality_witness_suite_report,
};

pub const TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_turing_completeness_closeout_audit_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarTuringCompletenessCloseoutStatus {
    TheoryGreenOperatorGreenServedSuppressed,
    Incomplete,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTuringCompletenessCloseoutSourceRow {
    pub claim_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarTuringCompletenessCloseoutAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_contract_ref: String,
    pub universal_machine_proof_report_ref: String,
    pub witness_suite_report_ref: String,
    pub minimal_universal_substrate_gate_report_ref: String,
    pub universality_verdict_split_report_ref: String,
    pub runtime_contract: TassadarTcmV1RuntimeContractReport,
    pub universal_machine_proof_report: TassadarUniversalMachineProofReport,
    pub witness_suite_report: TassadarUniversalityWitnessSuiteReport,
    pub minimal_universal_substrate_gate_report:
        TassadarMinimalUniversalSubstrateAcceptanceGateReport,
    pub universality_verdict_split_report: TassadarUniversalityVerdictSplitReport,
    pub source_rows: Vec<TassadarTuringCompletenessCloseoutSourceRow>,
    pub portability_envelope_ids: Vec<String>,
    pub refusal_boundary_ids: Vec<String>,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub claim_status: TassadarTuringCompletenessCloseoutStatus,
    pub allowed_statement: String,
    pub explicit_scope: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarTuringCompletenessCloseoutAuditReportError {
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error(transparent)]
    UniversalMachineProof(#[from] TassadarUniversalMachineProofReportError),
    #[error(transparent)]
    WitnessSuite(#[from] TassadarUniversalityWitnessSuiteReportError),
    #[error(transparent)]
    MinimalGate(#[from] TassadarMinimalUniversalSubstrateAcceptanceGateReportError),
    #[error(transparent)]
    VerdictSplit(#[from] TassadarUniversalityVerdictSplitReportError),
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

pub fn build_tassadar_turing_completeness_closeout_audit_report() -> Result<
    TassadarTuringCompletenessCloseoutAuditReport,
    TassadarTuringCompletenessCloseoutAuditReportError,
> {
    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let universal_machine_proof_report = build_tassadar_universal_machine_proof_report()?;
    let witness_suite_report = build_tassadar_universality_witness_suite_report()?;
    let minimal_universal_substrate_gate_report =
        build_tassadar_minimal_universal_substrate_acceptance_gate_report()?;
    let universality_verdict_split_report = build_tassadar_universality_verdict_split_report()?;

    let theory_green = universality_verdict_split_report.theory_green;
    let operator_green = universality_verdict_split_report.operator_green;
    let served_green = universality_verdict_split_report.served_green;

    let source_rows = vec![
        source_row(
            "declared_universal_substrate",
            runtime_contract.overall_green,
            vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            "the declared `TCM.v1` runtime contract must stay green before the closeout can make any Turing-completeness statement",
        ),
        source_row(
            "construction_backed_universal_machine_witness",
            universal_machine_proof_report.overall_green,
            vec![String::from(TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF)],
            "the explicit universal-machine witness construction must stay green before the closeout can make any Turing-completeness statement",
        ),
        source_row(
            "benchmark_bound_witness_suite",
            witness_suite_report.overall_green,
            vec![String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF)],
            "the dedicated witness suite must stay green before the closeout can make any Turing-completeness statement",
        ),
        source_row(
            "minimal_universal_substrate_gate",
            minimal_universal_substrate_gate_report.overall_green,
            vec![String::from(
                TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
            )],
            "the minimal universal-substrate gate must stay green before the closeout can make any Turing-completeness statement",
        ),
        source_row(
            "verdict_split_explicit",
            universality_verdict_split_report.overall_green,
            vec![String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF)],
            "the theory/operator/served verdict split must stay explicit before the closeout can make any Turing-completeness statement",
        ),
    ];

    let mut portability_envelope_ids = witness_suite_report
        .family_rows
        .iter()
        .map(|row| row.runtime_envelope.clone())
        .collect::<Vec<_>>();
    portability_envelope_ids.push(runtime_contract.runtime_envelope.clone());
    portability_envelope_ids.sort();
    portability_envelope_ids.dedup();

    let mut refusal_boundary_ids = runtime_contract.refused_out_of_model_semantic_ids.clone();
    refusal_boundary_ids.extend(
        witness_suite_report
            .family_rows
            .iter()
            .filter(|row| {
                row.expected_status
                    == psionic_data::TassadarUniversalityWitnessExpectation::RefusalBoundary
            })
            .map(|row| format!("{:?}", row.witness_family).to_lowercase())
            .collect::<Vec<_>>(),
    );
    refusal_boundary_ids.sort();
    refusal_boundary_ids.dedup();

    let claim_status = if source_rows.iter().all(|row| row.satisfied)
        && theory_green
        && operator_green
        && !served_green
    {
        TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed
    } else {
        TassadarTuringCompletenessCloseoutStatus::Incomplete
    };

    let allowed_statement = match claim_status {
        TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed => {
            String::from(
                "Psionic/Tassadar now supports Turing-complete computation under declared `TCM.v1` semantics for theory and operator use through bounded slices, persisted continuation, spill/tape extension, and explicit refusal boundaries. Served/public universality remains suppressed.",
            )
        }
        TassadarTuringCompletenessCloseoutStatus::Incomplete => String::from(
            "Psionic/Tassadar does not yet have enough green terminal-contract artifacts to make a bounded Turing-completeness statement.",
        ),
    };

    let explicit_scope = vec![
        String::from("declared_tcm_v1_substrate"),
        String::from("construction_backed_universal_machine_witnesses"),
        String::from("benchmark_bound_universality_witness_suite"),
        String::from("minimal_universal_substrate_gate"),
        String::from("operator_enveloped_resumable_execution"),
    ];
    let explicit_non_implications = vec![
        String::from("arbitrary Wasm execution"),
        String::from("broad served internal compute"),
        String::from("public universality publication"),
        String::from("ambient host effects"),
        String::from("settlement-qualified universality closure"),
        String::from("unrestricted portability beyond declared envelopes"),
    ];

    let mut report = TassadarTuringCompletenessCloseoutAuditReport {
        schema_version: 1,
        report_id: String::from("tassadar.turing_completeness_closeout_audit.report.v1"),
        runtime_contract_ref: String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
        universal_machine_proof_report_ref: String::from(
            TASSADAR_UNIVERSAL_MACHINE_PROOF_REPORT_REF,
        ),
        witness_suite_report_ref: String::from(TASSADAR_UNIVERSALITY_WITNESS_SUITE_REPORT_REF),
        minimal_universal_substrate_gate_report_ref: String::from(
            TASSADAR_MINIMAL_UNIVERSAL_SUBSTRATE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        universality_verdict_split_report_ref: String::from(
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        runtime_contract,
        universal_machine_proof_report,
        witness_suite_report,
        minimal_universal_substrate_gate_report,
        universality_verdict_split_report,
        source_rows,
        portability_envelope_ids,
        refusal_boundary_ids,
        theory_green,
        operator_green,
        served_green,
        claim_status,
        allowed_statement,
        explicit_scope,
        explicit_non_implications,
        claim_boundary: String::from(
            "this closeout audit is the final bounded terminal statement for Turing completeness inside standalone psionic. It refers only to the declared substrate, explicit witness constructions, the dedicated witness suite, the minimal gate, portability envelopes, refusal boundaries, and the explicit verdict split; it does not widen served/public posture.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Turing-completeness closeout audit keeps source_rows={}, portability_envelopes={}, refusal_boundaries={}, claim_status={:?}.",
        report.source_rows.len(),
        report.portability_envelope_ids.len(),
        report.refusal_boundary_ids.len(),
        report.claim_status,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_turing_completeness_closeout_audit_report|",
        &report,
    );
    Ok(report)
}

fn source_row(
    claim_id: &str,
    satisfied: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarTuringCompletenessCloseoutSourceRow {
    TassadarTuringCompletenessCloseoutSourceRow {
        claim_id: String::from(claim_id),
        satisfied,
        source_refs,
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_turing_completeness_closeout_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF)
}

pub fn write_tassadar_turing_completeness_closeout_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarTuringCompletenessCloseoutAuditReport,
    TassadarTuringCompletenessCloseoutAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarTuringCompletenessCloseoutAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_turing_completeness_closeout_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarTuringCompletenessCloseoutAuditReportError::Write {
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
) -> Result<T, TassadarTuringCompletenessCloseoutAuditReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarTuringCompletenessCloseoutAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarTuringCompletenessCloseoutAuditReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
        TassadarTuringCompletenessCloseoutAuditReport, TassadarTuringCompletenessCloseoutStatus,
        build_tassadar_turing_completeness_closeout_audit_report, read_json,
        tassadar_turing_completeness_closeout_audit_report_path,
        write_tassadar_turing_completeness_closeout_audit_report,
    };
    use tempfile::tempdir;

    #[test]
    fn turing_completeness_closeout_audit_keeps_scope_and_non_implications_explicit() {
        let report = build_tassadar_turing_completeness_closeout_audit_report().expect("report");

        assert_eq!(
            report.claim_status,
            TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed
        );
        assert!(
            report
                .explicit_non_implications
                .contains(&String::from("arbitrary Wasm execution"))
        );
        assert!(!report.served_green);
    }

    #[test]
    fn turing_completeness_closeout_audit_matches_committed_truth() {
        let generated = build_tassadar_turing_completeness_closeout_audit_report().expect("report");
        let committed: TassadarTuringCompletenessCloseoutAuditReport =
            read_json(tassadar_turing_completeness_closeout_audit_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_turing_completeness_closeout_audit_report.json"
        );
    }

    #[test]
    fn write_turing_completeness_closeout_audit_persists_current_truth() {
        let dir = tempdir().expect("tempdir");
        let output_path = dir
            .path()
            .join("tassadar_turing_completeness_closeout_audit_report.json");
        let report =
            write_tassadar_turing_completeness_closeout_audit_report(&output_path).expect("report");
        let reloaded: TassadarTuringCompletenessCloseoutAuditReport =
            read_json(&output_path).expect("reloaded");
        assert_eq!(report, reloaded);
    }
}
