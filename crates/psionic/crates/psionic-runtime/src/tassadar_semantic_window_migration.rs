use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TASSADAR_OPERATOR_PROPOSAL_LIFT_WINDOW_ID, TASSADAR_PUBLIC_PROPOSAL_LIFT_WINDOW_ID,
    TassadarSemanticWindowRevisionError, build_tassadar_semantic_window_revision_receipt,
    resolve_declared_tassadar_semantic_window,
};

pub const TASSADAR_SEMANTIC_WINDOW_MIGRATION_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_semantic_window_migration_report.json";

const TASSADAR_STALE_WINDOW_ID: &str = "tassadar.frozen_core_wasm.window.v0";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSemanticWindowMigrationStatus {
    Exact,
    Downgraded,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowMigrationCaseReceipt {
    pub case_id: String,
    pub requested_window_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_window_id: Option<String>,
    pub effective_profile_ids: Vec<String>,
    pub status: TassadarSemanticWindowMigrationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_receipt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSemanticWindowMigrationReport {
    pub schema_version: u16,
    pub report_id: String,
    pub revision_receipt_ref: String,
    pub active_window_id: String,
    pub exact_case_count: u32,
    pub downgraded_case_count: u32,
    pub refused_case_count: u32,
    pub exact_requested_window_ids: Vec<String>,
    pub downgraded_requested_window_ids: Vec<String>,
    pub refused_requested_window_ids: Vec<String>,
    pub case_receipts: Vec<TassadarSemanticWindowMigrationCaseReceipt>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSemanticWindowMigrationReportError {
    #[error(transparent)]
    Revision(#[from] TassadarSemanticWindowRevisionError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
}

#[must_use]
pub fn build_tassadar_semantic_window_migration_report() -> TassadarSemanticWindowMigrationReport {
    let revision_receipt = build_tassadar_semantic_window_revision_receipt();
    let active_window_id = revision_receipt.active_window.window_id.clone();
    let mut case_receipts = vec![
        TassadarSemanticWindowMigrationCaseReceipt {
            case_id: String::from("window.active.selected.v1"),
            requested_window_id: active_window_id.clone(),
            effective_window_id: Some(active_window_id.clone()),
            effective_profile_ids: Vec::new(),
            status: TassadarSemanticWindowMigrationStatus::Exact,
            migration_receipt_id: Some(String::from(
                "tassadar.semantic_window.migration_receipt.active_passthrough.v1",
            )),
            refusal_reason_id: None,
            detail: String::from(
                "the active frozen core-Wasm window remains selected exactly and does not require downgrade planning",
            ),
        },
        TassadarSemanticWindowMigrationCaseReceipt {
            case_id: String::from("window.metadata_refresh.forward.v1"),
            requested_window_id: String::from(crate::TASSADAR_METADATA_REFRESH_WINDOW_ID),
            effective_window_id: Some(active_window_id.clone()),
            effective_profile_ids: Vec::new(),
            status: TassadarSemanticWindowMigrationStatus::Exact,
            migration_receipt_id: Some(String::from(
                "tassadar.semantic_window.migration_receipt.metadata_refresh.v1",
            )),
            refusal_reason_id: None,
            detail: String::from(
                "the metadata-only candidate migrates to the active frozen core window as a no-op after rerunning the declared frozen-window drill",
            ),
        },
        TassadarSemanticWindowMigrationCaseReceipt {
            case_id: String::from("window.public_proposal_lift.downgrade.v1"),
            requested_window_id: String::from(TASSADAR_PUBLIC_PROPOSAL_LIFT_WINDOW_ID),
            effective_window_id: Some(active_window_id.clone()),
            effective_profile_ids: vec![
                String::from("tassadar.proposal_profile.exceptions_try_catch_rethrow.v1"),
                String::from("tassadar.proposal_profile.simd_deterministic.v1"),
            ],
            status: TassadarSemanticWindowMigrationStatus::Downgraded,
            migration_receipt_id: Some(String::from(
                "tassadar.semantic_window.migration_receipt.public_proposal_downgrade.v1",
            )),
            refusal_reason_id: None,
            detail: String::from(
                "public proposal-family widening does not become a new frozen window; the honest migration path downgrades to the active window plus explicit named exceptions and SIMD profiles",
            ),
        },
        TassadarSemanticWindowMigrationCaseReceipt {
            case_id: String::from("window.operator_proposal_lift.refuse.v1"),
            requested_window_id: String::from(TASSADAR_OPERATOR_PROPOSAL_LIFT_WINDOW_ID),
            effective_window_id: None,
            effective_profile_ids: Vec::new(),
            status: TassadarSemanticWindowMigrationStatus::Refused,
            migration_receipt_id: None,
            refusal_reason_id: Some(String::from("evidence_boundary")),
            detail: String::from(
                "operator-only proposal families remain blocked until each family graduates through its own evidence and publication posture; there is no honest cross-window downgrade here",
            ),
        },
        stale_window_refusal_case(),
    ];
    case_receipts.sort_by(|left, right| left.case_id.cmp(&right.case_id));

    let exact_requested_window_ids = case_receipts
        .iter()
        .filter(|case| case.status == TassadarSemanticWindowMigrationStatus::Exact)
        .map(|case| case.requested_window_id.clone())
        .collect::<Vec<_>>();
    let downgraded_requested_window_ids = case_receipts
        .iter()
        .filter(|case| case.status == TassadarSemanticWindowMigrationStatus::Downgraded)
        .map(|case| case.requested_window_id.clone())
        .collect::<Vec<_>>();
    let refused_requested_window_ids = case_receipts
        .iter()
        .filter(|case| case.status == TassadarSemanticWindowMigrationStatus::Refused)
        .map(|case| case.requested_window_id.clone())
        .collect::<Vec<_>>();

    let mut report = TassadarSemanticWindowMigrationReport {
        schema_version: 1,
        report_id: String::from("tassadar.semantic_window_migration.report.v1"),
        revision_receipt_ref: String::from(crate::TASSADAR_SEMANTIC_WINDOW_REVISION_RECEIPT_REF),
        active_window_id,
        exact_case_count: exact_requested_window_ids.len() as u32,
        downgraded_case_count: downgraded_requested_window_ids.len() as u32,
        refused_case_count: refused_requested_window_ids.len() as u32,
        exact_requested_window_ids,
        downgraded_requested_window_ids,
        refused_requested_window_ids,
        case_receipts,
        claim_boundary: String::from(
            "this report freezes one bounded cross-window migration lane for the frozen core-Wasm semantic window. It keeps exact active or metadata refresh migration, named-profile downgrade, and typed refusal explicit instead of implying one giant semantic superset or silent portability lift",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Semantic-window migration report keeps active_window_id={}, exact_cases={}, downgraded_cases={}, refused_cases={}.",
        report.active_window_id,
        report.exact_case_count,
        report.downgraded_case_count,
        report.refused_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_semantic_window_migration_report|",
        &report,
    );
    report
}

fn stale_window_refusal_case() -> TassadarSemanticWindowMigrationCaseReceipt {
    let detail = match resolve_declared_tassadar_semantic_window(TASSADAR_STALE_WINDOW_ID) {
        Err(TassadarSemanticWindowRevisionError::UndeclaredWindowId { .. }) => String::from(
            "stale frozen-window ids stay on typed undeclared-window refusal instead of silently shadowing the active window",
        ),
        Ok(_) | Err(_) => {
            String::from("stale frozen-window ids must stay on typed undeclared-window refusal")
        }
    };
    TassadarSemanticWindowMigrationCaseReceipt {
        case_id: String::from("window.stale_v0.refuse.v1"),
        requested_window_id: String::from(TASSADAR_STALE_WINDOW_ID),
        effective_window_id: None,
        effective_profile_ids: Vec::new(),
        status: TassadarSemanticWindowMigrationStatus::Refused,
        migration_receipt_id: None,
        refusal_reason_id: Some(String::from("undeclared_window_id")),
        detail,
    }
}

#[must_use]
pub fn tassadar_semantic_window_migration_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SEMANTIC_WINDOW_MIGRATION_REPORT_REF)
}

pub fn write_tassadar_semantic_window_migration_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSemanticWindowMigrationReport, TassadarSemanticWindowMigrationReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSemanticWindowMigrationReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_semantic_window_migration_report();
    let json = serde_json::to_string_pretty(&report).expect("report serializes");
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSemanticWindowMigrationReportError::Write {
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
mod tests {
    use super::{
        TassadarSemanticWindowMigrationReport, TassadarSemanticWindowMigrationStatus,
        build_tassadar_semantic_window_migration_report,
        write_tassadar_semantic_window_migration_report,
    };

    #[test]
    fn semantic_window_migration_report_keeps_exact_downgraded_and_refused_cases_explicit() {
        let report = build_tassadar_semantic_window_migration_report();

        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.downgraded_case_count, 1);
        assert_eq!(report.refused_case_count, 2);
        assert!(report.case_receipts.iter().any(|case| {
            case.requested_window_id == "tassadar.frozen_core_wasm.window.v1_plus_public_proposals"
                && case.status == TassadarSemanticWindowMigrationStatus::Downgraded
        }));
        assert!(report.case_receipts.iter().any(|case| {
            case.requested_window_id == "tassadar.frozen_core_wasm.window.v0"
                && case.refusal_reason_id.as_deref() == Some("undeclared_window_id")
        }));
    }

    #[test]
    fn write_semantic_window_migration_report_persists_current_truth() {
        let output_path =
            std::env::temp_dir().join("tassadar_semantic_window_migration_report.json");
        let report = write_tassadar_semantic_window_migration_report(&output_path)
            .expect("report should write");
        let bytes = std::fs::read(&output_path).expect("persisted report should exist");
        let persisted: TassadarSemanticWindowMigrationReport =
            serde_json::from_slice(&bytes).expect("persisted report should decode");
        assert_eq!(persisted, report);
        std::fs::remove_file(&output_path).expect("temp report should be removable");
    }
}
