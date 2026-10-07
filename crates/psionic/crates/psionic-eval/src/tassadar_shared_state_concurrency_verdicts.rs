use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_cluster::{
    TASSADAR_SHARED_STATE_CONCURRENCY_CHALLENGE_MATRIX_REPORT_REF,
    TassadarSharedStateConcurrencyChallengeMatrixReport,
    TassadarSharedStateConcurrencyChallengeStatus,
    build_tassadar_shared_state_concurrency_challenge_matrix_report,
};
use psionic_runtime::{
    TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF,
    TASSADAR_THREADS_RESEARCH_PROFILE_ID, TassadarSharedStateConcurrencyRuntimeStatus,
    TassadarSharedStateConcurrencyRuntimeVerdictReport,
    build_tassadar_shared_state_concurrency_runtime_verdict_report,
};

pub const TASSADAR_SHARED_STATE_CONCURRENCY_VERDICT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_state_concurrency_verdict_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedStateConcurrencyFinalVerdict {
    OperatorGreenPublicSuppressed,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedStateConcurrencyVerdictRow {
    pub concurrency_class_id: String,
    pub runtime_status: TassadarSharedStateConcurrencyRuntimeStatus,
    pub cluster_status: TassadarSharedStateConcurrencyChallengeStatus,
    pub final_verdict: TassadarSharedStateConcurrencyFinalVerdict,
    pub operator_truth_allowed: bool,
    pub public_truth_allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedStateConcurrencyVerdictReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_report_ref: String,
    pub runtime_report: TassadarSharedStateConcurrencyRuntimeVerdictReport,
    pub cluster_report_ref: String,
    pub cluster_report: TassadarSharedStateConcurrencyChallengeMatrixReport,
    pub rows: Vec<TassadarSharedStateConcurrencyVerdictRow>,
    pub operator_green_class_ids: Vec<String>,
    pub refused_class_ids: Vec<String>,
    pub operator_profile_allowed_profile_ids: Vec<String>,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSharedStateConcurrencyVerdictReportError {
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

#[must_use]
pub fn build_tassadar_shared_state_concurrency_verdict_report()
-> TassadarSharedStateConcurrencyVerdictReport {
    let runtime_report = build_tassadar_shared_state_concurrency_runtime_verdict_report();
    let cluster_report = build_tassadar_shared_state_concurrency_challenge_matrix_report();
    let cluster_rows = cluster_report
        .rows
        .iter()
        .map(|row| (row.concurrency_class_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let rows = runtime_report
        .rows
        .iter()
        .map(|runtime_row| {
            let cluster_row = cluster_rows
                .get(runtime_row.concurrency_class_id.as_str())
                .expect("cluster matrix should cover each runtime verdict row");
            let final_verdict = match (runtime_row.status, cluster_row.status) {
                (
                    TassadarSharedStateConcurrencyRuntimeStatus::OperatorDeterministicGreen,
                    TassadarSharedStateConcurrencyChallengeStatus::DeterministicGreen,
                ) => TassadarSharedStateConcurrencyFinalVerdict::OperatorGreenPublicSuppressed,
                _ => TassadarSharedStateConcurrencyFinalVerdict::Refused,
            };
            TassadarSharedStateConcurrencyVerdictRow {
                concurrency_class_id: runtime_row.concurrency_class_id.clone(),
                runtime_status: runtime_row.status,
                cluster_status: cluster_row.status,
                final_verdict,
                operator_truth_allowed: final_verdict
                    == TassadarSharedStateConcurrencyFinalVerdict::OperatorGreenPublicSuppressed,
                public_truth_allowed: false,
                refusal_reason_id: runtime_row
                    .refusal_reason_id
                    .clone()
                    .or_else(|| cluster_row.refusal_reason_id.clone()),
                note: format!("{} {}", runtime_row.note, cluster_row.note),
            }
        })
        .collect::<Vec<_>>();
    let operator_green_class_ids = rows
        .iter()
        .filter(|row| {
            row.final_verdict
                == TassadarSharedStateConcurrencyFinalVerdict::OperatorGreenPublicSuppressed
        })
        .map(|row| row.concurrency_class_id.clone())
        .collect::<Vec<_>>();
    let refused_class_ids = rows
        .iter()
        .filter(|row| row.final_verdict == TassadarSharedStateConcurrencyFinalVerdict::Refused)
        .map(|row| row.concurrency_class_id.clone())
        .collect::<Vec<_>>();
    let operator_profile_allowed_profile_ids =
        vec![String::from(TASSADAR_THREADS_RESEARCH_PROFILE_ID)];
    let public_profile_allowed_profile_ids = Vec::new();
    let mut report = TassadarSharedStateConcurrencyVerdictReport {
        schema_version: 1,
        report_id: String::from("tassadar.shared_state_concurrency.verdict_report.v1"),
        runtime_report_ref: String::from(
            TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF,
        ),
        runtime_report,
        cluster_report_ref: String::from(
            TASSADAR_SHARED_STATE_CONCURRENCY_CHALLENGE_MATRIX_REPORT_REF,
        ),
        cluster_report,
        rows,
        operator_green_class_ids,
        refused_class_ids,
        operator_profile_allowed_profile_ids,
        public_profile_allowed_profile_ids,
        overall_green: true,
        claim_boundary: String::from(
            "this eval report publishes shared-state concurrency verdicts by class. The current honest posture is operator-green only for the explicit single-host deterministic scheduler classes, with public publication still suppressed and broader shared-state families kept on refusal paths. It does not claim public threads publication, general concurrency closure, or arbitrary shared-state portability",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.operator_green_class_ids.len() == 2
        && report.refused_class_ids.len() == 3
        && report.operator_profile_allowed_profile_ids
            == vec![String::from(TASSADAR_THREADS_RESEARCH_PROFILE_ID)]
        && report.public_profile_allowed_profile_ids.is_empty();
    report.summary = format!(
        "Shared-state concurrency verdict report covers operator_green_classes={}, refused_classes={}, operator_profiles={}, public_profiles={}, overall_green={}.",
        report.operator_green_class_ids.len(),
        report.refused_class_ids.len(),
        report.operator_profile_allowed_profile_ids.len(),
        report.public_profile_allowed_profile_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_shared_state_concurrency_verdict_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_shared_state_concurrency_verdict_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SHARED_STATE_CONCURRENCY_VERDICT_REPORT_REF)
}

pub fn write_tassadar_shared_state_concurrency_verdict_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarSharedStateConcurrencyVerdictReport,
    TassadarSharedStateConcurrencyVerdictReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSharedStateConcurrencyVerdictReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_shared_state_concurrency_verdict_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSharedStateConcurrencyVerdictReportError::Write {
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
) -> Result<T, TassadarSharedStateConcurrencyVerdictReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarSharedStateConcurrencyVerdictReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSharedStateConcurrencyVerdictReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarSharedStateConcurrencyFinalVerdict, TassadarSharedStateConcurrencyVerdictReport,
        build_tassadar_shared_state_concurrency_verdict_report, read_json,
        tassadar_shared_state_concurrency_verdict_report_path,
        write_tassadar_shared_state_concurrency_verdict_report,
    };
    use tempfile::tempdir;

    #[test]
    fn shared_state_concurrency_verdict_report_keeps_operator_and_public_posture_separate() {
        let report = build_tassadar_shared_state_concurrency_verdict_report();

        assert!(report.overall_green);
        assert_eq!(report.operator_green_class_ids.len(), 2);
        assert_eq!(report.refused_class_ids.len(), 3);
        assert!(report.public_profile_allowed_profile_ids.is_empty());
        assert!(report.rows.iter().any(|row| {
            row.final_verdict
                == TassadarSharedStateConcurrencyFinalVerdict::OperatorGreenPublicSuppressed
                && row.concurrency_class_id == "single_host_round_robin_shared_counter"
        }));
    }

    #[test]
    fn shared_state_concurrency_verdict_report_matches_committed_truth() {
        let generated = build_tassadar_shared_state_concurrency_verdict_report();
        let committed: TassadarSharedStateConcurrencyVerdictReport =
            read_json(tassadar_shared_state_concurrency_verdict_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_shared_state_concurrency_verdict_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_shared_state_concurrency_verdict_report.json");
        let report = write_tassadar_shared_state_concurrency_verdict_report(&output_path)
            .expect("write report");
        let persisted: TassadarSharedStateConcurrencyVerdictReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(report, persisted);
    }
}
