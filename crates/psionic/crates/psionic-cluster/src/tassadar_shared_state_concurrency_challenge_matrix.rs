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
    TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF, TassadarPreemptiveJobFairnessReport,
    build_tassadar_preemptive_job_fairness_report,
};
use psionic_runtime::{
    TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF,
    TassadarSharedStateConcurrencyRuntimeStatus,
    TassadarSharedStateConcurrencyRuntimeVerdictReport,
    build_tassadar_shared_state_concurrency_runtime_verdict_report,
};

pub const TASSADAR_SHARED_STATE_CONCURRENCY_CHALLENGE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_shared_state_concurrency_challenge_matrix_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSharedStateConcurrencyChallengeStatus {
    DeterministicGreen,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedStateConcurrencyChallengeRow {
    pub concurrency_class_id: String,
    pub cluster_scheduler_id: String,
    pub challenge_scope_id: String,
    pub status: TassadarSharedStateConcurrencyChallengeStatus,
    pub replay_green: bool,
    pub race_detected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub benchmark_refs: Vec<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSharedStateConcurrencyChallengeMatrixReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_verdict_report_ref: String,
    pub runtime_verdict_report: TassadarSharedStateConcurrencyRuntimeVerdictReport,
    pub fairness_report_ref: String,
    pub fairness_report: TassadarPreemptiveJobFairnessReport,
    pub green_concurrency_class_ids: Vec<String>,
    pub refused_concurrency_class_ids: Vec<String>,
    pub rows: Vec<TassadarSharedStateConcurrencyChallengeRow>,
    pub overall_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSharedStateConcurrencyChallengeMatrixReportError {
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
pub fn build_tassadar_shared_state_concurrency_challenge_matrix_report()
-> TassadarSharedStateConcurrencyChallengeMatrixReport {
    let runtime_verdict_report = build_tassadar_shared_state_concurrency_runtime_verdict_report();
    let fairness_report = build_tassadar_preemptive_job_fairness_report();
    let rows = vec![
        challenge_row(
            "single_host_round_robin_shared_counter",
            "deterministic_round_robin",
            "scheduler_order_audit",
            TassadarSharedStateConcurrencyChallengeStatus::DeterministicGreen,
            false,
            None,
            &[
                TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF,
                TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF,
            ],
            "single-host round-robin shared-state replay stays deterministic when scheduler order and replay are both explicit",
        ),
        challenge_row(
            "single_host_barrier_reduce",
            "deterministic_round_robin",
            "barrier_release_audit",
            TassadarSharedStateConcurrencyChallengeStatus::DeterministicGreen,
            false,
            None,
            &[
                TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF,
                TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF,
            ],
            "single-host barrier release plus reduce stays deterministic only inside the same explicit scheduler envelope",
        ),
        challenge_row(
            "host_nondeterministic_shared_counter",
            "host_nondeterministic_scheduler",
            "race_vs_replay_audit",
            TassadarSharedStateConcurrencyChallengeStatus::Refused,
            true,
            Some("host_nondeterministic_scheduler"),
            &[
                TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF,
                TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF,
            ],
            "host-nondeterministic scheduling stays refused because replay and race behavior cannot be flattened into one deterministic class",
        ),
        challenge_row(
            "relaxed_memory_order_shared_counter",
            "deterministic_round_robin",
            "memory_order_audit",
            TassadarSharedStateConcurrencyChallengeStatus::Refused,
            true,
            Some("unsupported_memory_ordering"),
            &[TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF],
            "relaxed memory ordering stays refused even when the scheduler is fixed because the memory semantics themselves are not frozen",
        ),
        challenge_row(
            "cross_worker_shared_heap_replication",
            "weighted_fair_slice_rotation",
            "cross_worker_shared_heap_audit",
            TassadarSharedStateConcurrencyChallengeStatus::Refused,
            true,
            Some("cross_worker_shared_state_unfrozen"),
            &[
                TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF,
                "fixtures/tassadar/reports/tassadar_linked_program_bundle_report.json",
            ],
            "cross-worker shared heaps stay refused because the repo still has a documented shared-state gap instead of a frozen deterministic cluster semantics claim",
        ),
    ];
    let green_concurrency_class_ids = rows
        .iter()
        .filter(|row| {
            row.status == TassadarSharedStateConcurrencyChallengeStatus::DeterministicGreen
        })
        .map(|row| row.concurrency_class_id.clone())
        .collect::<Vec<_>>();
    let refused_concurrency_class_ids = rows
        .iter()
        .filter(|row| row.status == TassadarSharedStateConcurrencyChallengeStatus::Refused)
        .map(|row| row.concurrency_class_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarSharedStateConcurrencyChallengeMatrixReport {
        schema_version: 1,
        report_id: String::from("tassadar.shared_state_concurrency.challenge_matrix.report.v1"),
        runtime_verdict_report_ref: String::from(
            TASSADAR_SHARED_STATE_CONCURRENCY_RUNTIME_VERDICT_REPORT_REF,
        ),
        runtime_verdict_report,
        fairness_report_ref: String::from(TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF),
        fairness_report,
        green_concurrency_class_ids,
        refused_concurrency_class_ids,
        rows,
        overall_green: true,
        claim_boundary: String::from(
            "this cluster report freezes one shared-state challenge matrix over explicit deterministic and refused concurrency classes. It keeps the current operator-green single-host classes visible, but host-nondeterministic scheduling, relaxed memory ordering, and cross-worker shared heaps remain explicit refusal truth. It does not imply served threads publication, broad cluster concurrency closure, or arbitrary shared-state portability",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.overall_green = report.green_concurrency_class_ids.len() == 2
        && report.refused_concurrency_class_ids.len() == 3
        && report.runtime_verdict_report.operator_green_class_ids.len() == 2
        && report
            .runtime_verdict_report
            .rows
            .iter()
            .all(|row| match row.status {
                TassadarSharedStateConcurrencyRuntimeStatus::OperatorDeterministicGreen => report
                    .green_concurrency_class_ids
                    .contains(&row.concurrency_class_id),
                TassadarSharedStateConcurrencyRuntimeStatus::Refused => report
                    .refused_concurrency_class_ids
                    .contains(&row.concurrency_class_id),
            });
    report.summary = format!(
        "Shared-state concurrency challenge matrix covers green_classes={}, refused_classes={}, runtime_operator_green_classes={}, overall_green={}.",
        report.green_concurrency_class_ids.len(),
        report.refused_concurrency_class_ids.len(),
        report.runtime_verdict_report.operator_green_class_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_shared_state_concurrency_challenge_matrix_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_shared_state_concurrency_challenge_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SHARED_STATE_CONCURRENCY_CHALLENGE_MATRIX_REPORT_REF)
}

pub fn write_tassadar_shared_state_concurrency_challenge_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarSharedStateConcurrencyChallengeMatrixReport,
    TassadarSharedStateConcurrencyChallengeMatrixReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSharedStateConcurrencyChallengeMatrixReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_shared_state_concurrency_challenge_matrix_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSharedStateConcurrencyChallengeMatrixReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn challenge_row(
    concurrency_class_id: &str,
    cluster_scheduler_id: &str,
    challenge_scope_id: &str,
    status: TassadarSharedStateConcurrencyChallengeStatus,
    race_detected: bool,
    refusal_reason_id: Option<&str>,
    benchmark_refs: &[&str],
    note: &str,
) -> TassadarSharedStateConcurrencyChallengeRow {
    TassadarSharedStateConcurrencyChallengeRow {
        concurrency_class_id: String::from(concurrency_class_id),
        cluster_scheduler_id: String::from(cluster_scheduler_id),
        challenge_scope_id: String::from(challenge_scope_id),
        status,
        replay_green: status == TassadarSharedStateConcurrencyChallengeStatus::DeterministicGreen,
        race_detected,
        refusal_reason_id: refusal_reason_id.map(String::from),
        benchmark_refs: benchmark_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        note: String::from(note),
    }
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
) -> Result<T, TassadarSharedStateConcurrencyChallengeMatrixReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSharedStateConcurrencyChallengeMatrixReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSharedStateConcurrencyChallengeMatrixReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarSharedStateConcurrencyChallengeMatrixReport,
        TassadarSharedStateConcurrencyChallengeStatus,
        build_tassadar_shared_state_concurrency_challenge_matrix_report, read_json,
        tassadar_shared_state_concurrency_challenge_matrix_report_path,
        write_tassadar_shared_state_concurrency_challenge_matrix_report,
    };
    use tempfile::tempdir;

    #[test]
    fn shared_state_concurrency_challenge_matrix_keeps_green_and_refused_classes_explicit() {
        let report = build_tassadar_shared_state_concurrency_challenge_matrix_report();

        assert!(report.overall_green);
        assert_eq!(report.green_concurrency_class_ids.len(), 2);
        assert_eq!(report.refused_concurrency_class_ids.len(), 3);
        assert!(report.rows.iter().any(|row| {
            row.status == TassadarSharedStateConcurrencyChallengeStatus::Refused
                && row.concurrency_class_id == "cross_worker_shared_heap_replication"
        }));
    }

    #[test]
    fn shared_state_concurrency_challenge_matrix_matches_committed_truth() {
        let generated = build_tassadar_shared_state_concurrency_challenge_matrix_report();
        let committed: TassadarSharedStateConcurrencyChallengeMatrixReport =
            read_json(tassadar_shared_state_concurrency_challenge_matrix_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_shared_state_concurrency_challenge_matrix_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_shared_state_concurrency_challenge_matrix_report.json");
        let report = write_tassadar_shared_state_concurrency_challenge_matrix_report(&output_path)
            .expect("write report");
        let persisted: TassadarSharedStateConcurrencyChallengeMatrixReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(report, persisted);
    }
}
