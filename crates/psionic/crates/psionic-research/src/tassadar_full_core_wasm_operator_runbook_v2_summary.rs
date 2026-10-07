use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_eval::{
    build_tassadar_full_core_wasm_public_acceptance_gate_report,
    TassadarFullCoreWasmPublicAcceptanceGateReport,
    TassadarFullCoreWasmPublicAcceptanceGateReportError,
};

pub const TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_SUMMARY_REF: &str =
    "fixtures/tassadar/reports/tassadar_full_core_wasm_operator_runbook_v2_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarFullCoreWasmOperatorRunbookV2Summary {
    pub schema_version: u16,
    pub report_id: String,
    pub eval_report: TassadarFullCoreWasmPublicAcceptanceGateReport,
    pub allowed_statement: String,
    pub blocked_by: Vec<String>,
    pub explicit_non_implications: Vec<String>,
    pub operator_drill_commands: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarFullCoreWasmOperatorRunbookV2SummaryError {
    #[error(transparent)]
    Eval(#[from] TassadarFullCoreWasmPublicAcceptanceGateReportError),
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

pub fn build_tassadar_full_core_wasm_operator_runbook_v2_summary() -> Result<
    TassadarFullCoreWasmOperatorRunbookV2Summary,
    TassadarFullCoreWasmOperatorRunbookV2SummaryError,
> {
    let eval_report = build_tassadar_full_core_wasm_public_acceptance_gate_report()?;
    let blocked_by = eval_report.suppressed_requirement_ids.clone();
    let explicit_non_implications = vec![
        String::from("arbitrary Wasm execution"),
        String::from("post-core proposal-family closure"),
        String::from("broad internal compute"),
        String::from("Turing-complete support"),
    ];
    let mut summary = TassadarFullCoreWasmOperatorRunbookV2Summary {
        schema_version: 1,
        report_id: String::from("tassadar.full_core_wasm_operator_runbook_v2.summary.v1"),
        allowed_statement: String::from(
            "Psionic/Tassadar has one declared frozen core-Wasm target with a machine-readable closure verdict and an operator drill, but the repo may not yet claim full core-Wasm public closure because feature-family coverage, cross-machine replay, and served publication remain blocked.",
        ),
        operator_drill_commands: eval_report.operator_drill_commands.clone(),
        blocked_by,
        explicit_non_implications,
        eval_report,
        claim_boundary: String::from(
            "this summary turns the full core-Wasm public acceptance gate into one operator-facing runbook summary. It explains how to rerun the gate and what still blocks public closure without widening the claim boundary beyond the committed gate artifact",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    summary.summary = format!(
        "Full core-Wasm operator runbook v2 summary records blocked_by={}, explicit_non_implications={}, operator_drill_commands={}, acceptance_status={:?}.",
        summary.blocked_by.len(),
        summary.explicit_non_implications.len(),
        summary.operator_drill_commands.len(),
        summary.eval_report.acceptance_status,
    );
    summary.report_digest = stable_digest(
        b"psionic_tassadar_full_core_wasm_operator_runbook_v2_summary|",
        &summary,
    );
    Ok(summary)
}

#[must_use]
pub fn tassadar_full_core_wasm_operator_runbook_v2_summary_path() -> PathBuf {
    repo_root().join(TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_SUMMARY_REF)
}

pub fn write_tassadar_full_core_wasm_operator_runbook_v2_summary(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarFullCoreWasmOperatorRunbookV2Summary,
    TassadarFullCoreWasmOperatorRunbookV2SummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarFullCoreWasmOperatorRunbookV2SummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let summary = build_tassadar_full_core_wasm_operator_runbook_v2_summary()?;
    let json = serde_json::to_string_pretty(&summary)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarFullCoreWasmOperatorRunbookV2SummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(summary)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-research crate dir")
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
) -> Result<T, TassadarFullCoreWasmOperatorRunbookV2SummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarFullCoreWasmOperatorRunbookV2SummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarFullCoreWasmOperatorRunbookV2SummaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_full_core_wasm_operator_runbook_v2_summary, read_repo_json,
        tassadar_full_core_wasm_operator_runbook_v2_summary_path,
        TassadarFullCoreWasmOperatorRunbookV2Summary,
        TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_SUMMARY_REF,
    };

    #[test]
    fn full_core_wasm_operator_runbook_v2_summary_keeps_blockers_explicit() {
        let summary = build_tassadar_full_core_wasm_operator_runbook_v2_summary().expect("summary");

        assert!(summary
            .blocked_by
            .contains(&String::from("target_feature_family_coverage")));
        assert!(summary
            .blocked_by
            .contains(&String::from("cross_machine_harness_replay")));
        assert!(summary
            .explicit_non_implications
            .contains(&String::from("Turing-complete support")));
    }

    #[test]
    fn full_core_wasm_operator_runbook_v2_summary_matches_committed_truth() {
        let generated =
            build_tassadar_full_core_wasm_operator_runbook_v2_summary().expect("summary");
        let committed: TassadarFullCoreWasmOperatorRunbookV2Summary =
            read_repo_json(TASSADAR_FULL_CORE_WASM_OPERATOR_RUNBOOK_V2_SUMMARY_REF)
                .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn full_core_wasm_operator_runbook_v2_summary_path_is_stable() {
        assert_eq!(
            tassadar_full_core_wasm_operator_runbook_v2_summary_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_full_core_wasm_operator_runbook_v2_summary.json")
        );
    }
}
