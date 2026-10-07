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
    build_tassadar_frozen_core_wasm_closure_gate_report,
    build_tassadar_frozen_core_wasm_window_report, TassadarFrozenCoreWasmClosureGateReportError,
    TassadarFrozenCoreWasmWindowReportError, TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF,
    TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF,
};
use psionic_runtime::TassadarFrozenCoreWasmClosureGateRowStatus;

pub const TASSADAR_FULL_CORE_WASM_PUBLIC_ACCEPTANCE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_full_core_wasm_public_acceptance_gate_report.json";
pub const TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_REF: &str = "docs/TASSADAR_WASM_RUNBOOK.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarFullCoreWasmPublicAcceptanceStatus {
    Green,
    Suppressed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFullCoreWasmPublicAcceptanceRow {
    pub requirement_id: String,
    pub status: TassadarFullCoreWasmPublicAcceptanceStatus,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFullCoreWasmPublicAcceptanceGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub frozen_window_report_ref: String,
    pub closure_gate_report_ref: String,
    pub operator_runbook_ref: String,
    pub operator_drill_commands: Vec<String>,
    pub rows: Vec<TassadarFullCoreWasmPublicAcceptanceRow>,
    pub green_requirement_ids: Vec<String>,
    pub suppressed_requirement_ids: Vec<String>,
    pub failed_requirement_ids: Vec<String>,
    pub acceptance_status: TassadarFullCoreWasmPublicAcceptanceStatus,
    pub served_publication_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarFullCoreWasmPublicAcceptanceGateReportError {
    #[error(transparent)]
    FrozenWindow(#[from] TassadarFrozenCoreWasmWindowReportError),
    #[error(transparent)]
    ClosureGate(#[from] TassadarFrozenCoreWasmClosureGateReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_full_core_wasm_public_acceptance_gate_report() -> Result<
    TassadarFullCoreWasmPublicAcceptanceGateReport,
    TassadarFullCoreWasmPublicAcceptanceGateReportError,
> {
    let window_report = build_tassadar_frozen_core_wasm_window_report()?;
    let closure_gate = build_tassadar_frozen_core_wasm_closure_gate_report()?;
    let operator_runbook_exists = repo_root()
        .join(TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_REF)
        .exists();
    let operator_drill_commands = vec![
        String::from("cargo run -p psionic-eval --example tassadar_frozen_core_wasm_window_report"),
        String::from(
            "cargo run -p psionic-eval --example tassadar_frozen_core_wasm_closure_gate_report",
        ),
        String::from(
            "cargo run -p psionic-eval --example tassadar_full_core_wasm_public_acceptance_gate_report",
        ),
        String::from(
            "cargo run -p psionic-research --example tassadar_full_core_wasm_operator_runbook_v2_summary",
        ),
        String::from(
            "cargo test -p psionic-eval frozen_core_wasm_window -- --nocapture",
        ),
        String::from(
            "cargo test -p psionic-eval frozen_core_wasm_closure_gate -- --nocapture",
        ),
        String::from(
            "cargo test -p psionic-eval full_core_wasm_public_acceptance_gate -- --nocapture",
        ),
    ];

    let mut rows = vec![
        row_from_closure_gate(
            &closure_gate,
            "official_window_and_harness",
            false,
            &[
                TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF,
                TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF,
            ],
        ),
        row_from_closure_gate(
            &closure_gate,
            "differential_execution_parity",
            false,
            &[TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF],
        ),
        row_from_closure_gate(
            &closure_gate,
            "trap_and_refusal_parity",
            false,
            &[TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF],
        ),
        row_from_closure_gate(
            &closure_gate,
            "target_feature_family_coverage",
            true,
            &[
                TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF,
                TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF,
            ],
        ),
        row_from_closure_gate(
            &closure_gate,
            "cross_machine_harness_replay",
            true,
            &[TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF],
        ),
        TassadarFullCoreWasmPublicAcceptanceRow {
            requirement_id: String::from("served_publication_gate"),
            status: if closure_gate.served_publication_allowed {
                TassadarFullCoreWasmPublicAcceptanceStatus::Green
            } else {
                TassadarFullCoreWasmPublicAcceptanceStatus::Suppressed
            },
            source_refs: vec![String::from(
                TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF,
            )],
            detail: if closure_gate.served_publication_allowed {
                String::from(
                    "served publication is allowed because every declared frozen core-Wasm gate row is green",
                )
            } else {
                String::from(
                    "served publication remains suppressed until every declared frozen core-Wasm gate row is green",
                )
            },
        },
        TassadarFullCoreWasmPublicAcceptanceRow {
            requirement_id: String::from("operator_runbook_v2"),
            status: if operator_runbook_exists && !operator_drill_commands.is_empty() {
                TassadarFullCoreWasmPublicAcceptanceStatus::Green
            } else {
                TassadarFullCoreWasmPublicAcceptanceStatus::Failed
            },
            source_refs: vec![String::from(
                TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_REF,
            )],
            detail: if operator_runbook_exists && !operator_drill_commands.is_empty() {
                format!(
                    "operator runbook v2 is declared at `{}` with {} explicit drill commands",
                    TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_REF,
                    operator_drill_commands.len()
                )
            } else {
                format!(
                    "operator runbook v2 is incomplete because `{}` is missing or has no declared drill commands",
                    TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_REF
                )
            },
        },
    ];
    rows.sort_by(|left, right| left.requirement_id.cmp(&right.requirement_id));

    let green_requirement_ids = rows
        .iter()
        .filter(|row| row.status == TassadarFullCoreWasmPublicAcceptanceStatus::Green)
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let suppressed_requirement_ids = rows
        .iter()
        .filter(|row| row.status == TassadarFullCoreWasmPublicAcceptanceStatus::Suppressed)
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();
    let failed_requirement_ids = rows
        .iter()
        .filter(|row| row.status == TassadarFullCoreWasmPublicAcceptanceStatus::Failed)
        .map(|row| row.requirement_id.clone())
        .collect::<Vec<_>>();

    let acceptance_status =
        if failed_requirement_ids.is_empty() && suppressed_requirement_ids.is_empty() {
            TassadarFullCoreWasmPublicAcceptanceStatus::Green
        } else if !failed_requirement_ids.is_empty() {
            TassadarFullCoreWasmPublicAcceptanceStatus::Failed
        } else {
            TassadarFullCoreWasmPublicAcceptanceStatus::Suppressed
        };
    let served_publication_allowed =
        acceptance_status == TassadarFullCoreWasmPublicAcceptanceStatus::Green;

    let mut report = TassadarFullCoreWasmPublicAcceptanceGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.full_core_wasm_public_acceptance_gate.report.v1"),
        frozen_window_report_ref: String::from(TASSADAR_FROZEN_CORE_WASM_WINDOW_REPORT_REF),
        closure_gate_report_ref: String::from(TASSADAR_FROZEN_CORE_WASM_CLOSURE_GATE_REPORT_REF),
        operator_runbook_ref: String::from(TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_REF),
        operator_drill_commands,
        rows,
        green_requirement_ids,
        suppressed_requirement_ids,
        failed_requirement_ids,
        acceptance_status,
        served_publication_allowed,
        claim_boundary: format!(
            "this gate is the disclosure-safe public acceptance gate for one frozen core-Wasm target. It binds the frozen semantic window, closure verdict, served-publication posture, and operator runbook v2 into one machine-readable verdict. It does not imply arbitrary Wasm execution, post-core proposal-family support, broad internal compute, or Turing-complete support. Current target window id: `{}`.",
            window_report.frozen_window.window_id
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Full core-Wasm public acceptance gate now records green_requirements={}, suppressed_requirements={}, failed_requirements={}, acceptance_status={:?}, served_publication_allowed={}.",
        report.green_requirement_ids.len(),
        report.suppressed_requirement_ids.len(),
        report.failed_requirement_ids.len(),
        report.acceptance_status,
        report.served_publication_allowed,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_full_core_wasm_public_acceptance_gate_report|",
        &report,
    );
    Ok(report)
}

fn row_from_closure_gate(
    closure_gate: &crate::TassadarFrozenCoreWasmClosureGateReport,
    row_id: &str,
    red_is_suppressed: bool,
    source_refs: &[&str],
) -> TassadarFullCoreWasmPublicAcceptanceRow {
    let gate_row = closure_gate
        .gate_rows
        .iter()
        .find(|row| row.row_id == row_id)
        .expect("closure gate rows should stay stable for the public acceptance gate");
    let status = match gate_row.status {
        TassadarFrozenCoreWasmClosureGateRowStatus::Green => {
            TassadarFullCoreWasmPublicAcceptanceStatus::Green
        }
        TassadarFrozenCoreWasmClosureGateRowStatus::Red if red_is_suppressed => {
            TassadarFullCoreWasmPublicAcceptanceStatus::Suppressed
        }
        TassadarFrozenCoreWasmClosureGateRowStatus::Red => {
            TassadarFullCoreWasmPublicAcceptanceStatus::Failed
        }
    };
    TassadarFullCoreWasmPublicAcceptanceRow {
        requirement_id: String::from(row_id),
        status,
        source_refs: source_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: gate_row.detail.clone(),
    }
}

#[must_use]
pub fn tassadar_full_core_wasm_public_acceptance_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_FULL_CORE_WASM_PUBLIC_ACCEPTANCE_GATE_REPORT_REF)
}

pub fn write_tassadar_full_core_wasm_public_acceptance_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarFullCoreWasmPublicAcceptanceGateReport,
    TassadarFullCoreWasmPublicAcceptanceGateReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarFullCoreWasmPublicAcceptanceGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_full_core_wasm_public_acceptance_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarFullCoreWasmPublicAcceptanceGateReportError::Write {
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

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarFullCoreWasmPublicAcceptanceGateReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarFullCoreWasmPublicAcceptanceGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarFullCoreWasmPublicAcceptanceGateReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
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
        build_tassadar_full_core_wasm_public_acceptance_gate_report, read_json,
        tassadar_full_core_wasm_public_acceptance_gate_report_path,
        TassadarFullCoreWasmPublicAcceptanceGateReport, TassadarFullCoreWasmPublicAcceptanceStatus,
    };

    #[test]
    fn full_core_wasm_public_acceptance_gate_keeps_current_blockers_explicit() {
        let report = build_tassadar_full_core_wasm_public_acceptance_gate_report().expect("report");

        assert_eq!(
            report.acceptance_status,
            TassadarFullCoreWasmPublicAcceptanceStatus::Suppressed
        );
        assert!(report.failed_requirement_ids.is_empty());
        assert!(report
            .suppressed_requirement_ids
            .contains(&String::from("target_feature_family_coverage")));
        assert!(report
            .suppressed_requirement_ids
            .contains(&String::from("cross_machine_harness_replay")));
        assert!(report
            .suppressed_requirement_ids
            .contains(&String::from("served_publication_gate")));
        assert!(!report.served_publication_allowed);
    }

    #[test]
    fn full_core_wasm_public_acceptance_gate_matches_committed_truth() {
        let generated =
            build_tassadar_full_core_wasm_public_acceptance_gate_report().expect("report");
        let committed: TassadarFullCoreWasmPublicAcceptanceGateReport =
            read_json(tassadar_full_core_wasm_public_acceptance_gate_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }
}
