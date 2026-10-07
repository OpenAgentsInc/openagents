use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TASSADAR_PREEMPTIVE_JOB_BUNDLE_FILE, TASSADAR_PREEMPTIVE_JOB_RUN_ROOT_REF,
    TassadarPreemptiveJobRuntimeBundle, TassadarPreemptiveJobStatus,
    build_tassadar_preemptive_job_runtime_bundle,
};

/// Stable committed report ref for bounded preemptive-job fairness truth.
pub const TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_preemptive_job_fairness_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPreemptiveJobSchedulerStatus {
    Green,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveJobFairnessRow {
    pub scheduler_id: String,
    pub status: TassadarPreemptiveJobSchedulerStatus,
    pub case_count: u32,
    pub max_consecutive_wait_slices: u32,
    pub max_preemption_count: u32,
    pub starvation_free: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveJobFairnessReport {
    pub schema_version: u16,
    pub report_id: String,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarPreemptiveJobRuntimeBundle,
    pub rows: Vec<TassadarPreemptiveJobFairnessRow>,
    pub green_scheduler_ids: Vec<String>,
    pub refused_scheduler_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub starvation_free_case_count: u32,
    pub fairness_window_green_case_count: u32,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPreemptiveJobFairnessReportError {
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

pub fn build_tassadar_preemptive_job_fairness_report() -> TassadarPreemptiveJobFairnessReport {
    let runtime_bundle = build_tassadar_preemptive_job_runtime_bundle();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_PREEMPTIVE_JOB_RUN_ROOT_REF, TASSADAR_PREEMPTIVE_JOB_BUNDLE_FILE
    );
    let rows = vec![
        scheduler_row(
            "deterministic_round_robin",
            TassadarPreemptiveJobSchedulerStatus::Green,
            &runtime_bundle,
            "deterministic round-robin keeps one-slice wait bounds and exact resumable parity",
        ),
        scheduler_row(
            "weighted_fair_slice_rotation",
            TassadarPreemptiveJobSchedulerStatus::Green,
            &runtime_bundle,
            "weighted fair slice rotation keeps bounded wait and exact resumable parity on the seeded search workload",
        ),
        scheduler_row(
            "host_nondeterministic_scheduler",
            TassadarPreemptiveJobSchedulerStatus::Refused,
            &runtime_bundle,
            "host nondeterministic scheduling remains explicitly refused for the bounded preemptive-job lane",
        ),
    ];
    let green_scheduler_ids = rows
        .iter()
        .filter(|row| row.status == TassadarPreemptiveJobSchedulerStatus::Green)
        .map(|row| row.scheduler_id.clone())
        .collect::<Vec<_>>();
    let refused_scheduler_ids = rows
        .iter()
        .filter(|row| row.status == TassadarPreemptiveJobSchedulerStatus::Refused)
        .map(|row| row.scheduler_id.clone())
        .collect::<Vec<_>>();
    let exact_case_count = runtime_bundle.exact_case_count;
    let refusal_case_count = runtime_bundle.refusal_case_count;
    let starvation_free_case_count = runtime_bundle
        .case_receipts
        .iter()
        .filter(|case| case.starvation_free)
        .count() as u32;
    let fairness_window_green_case_count = runtime_bundle
        .case_receipts
        .iter()
        .filter(|case| {
            case.status == TassadarPreemptiveJobStatus::ExactSliceBoundaryParity
                && case.max_consecutive_wait_slices <= case.fairness_window_slices
        })
        .count() as u32;
    let mut report = TassadarPreemptiveJobFairnessReport {
        schema_version: 1,
        report_id: String::from("tassadar.preemptive_job_fairness.report.v1"),
        runtime_bundle_ref,
        runtime_bundle,
        rows,
        green_scheduler_ids,
        refused_scheduler_ids,
        exact_case_count,
        refusal_case_count,
        starvation_free_case_count,
        fairness_window_green_case_count,
        claim_boundary: String::from(
            "this cluster report covers one bounded set of scheduler regimes for the preemptive-job lane only. It keeps host-nondeterministic or otherwise unverified fairness regimes on explicit refusal paths and does not imply arbitrary cluster scheduling, concurrent executor closure, or broader served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Preemptive-job fairness report covers green_schedulers={}, refused_schedulers={}, exact_cases={}, refusal_rows={}.",
        report.green_scheduler_ids.len(),
        report.refused_scheduler_ids.len(),
        report.exact_case_count,
        report.refusal_case_count,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_preemptive_job_fairness_report|", &report);
    report
}

#[must_use]
pub fn tassadar_preemptive_job_fairness_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF)
}

pub fn write_tassadar_preemptive_job_fairness_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPreemptiveJobFairnessReport, TassadarPreemptiveJobFairnessReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPreemptiveJobFairnessReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_preemptive_job_fairness_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPreemptiveJobFairnessReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn scheduler_row(
    scheduler_id: &str,
    status: TassadarPreemptiveJobSchedulerStatus,
    runtime_bundle: &TassadarPreemptiveJobRuntimeBundle,
    note: &str,
) -> TassadarPreemptiveJobFairnessRow {
    let matching_cases = runtime_bundle
        .case_receipts
        .iter()
        .filter(|case| case.scheduler_id == scheduler_id)
        .collect::<Vec<_>>();
    TassadarPreemptiveJobFairnessRow {
        scheduler_id: String::from(scheduler_id),
        status,
        case_count: matching_cases.len() as u32,
        max_consecutive_wait_slices: matching_cases
            .iter()
            .map(|case| case.max_consecutive_wait_slices)
            .max()
            .unwrap_or_default(),
        max_preemption_count: matching_cases
            .iter()
            .map(|case| case.preemption_count)
            .max()
            .unwrap_or_default(),
        starvation_free: matching_cases.iter().all(|case| case.starvation_free),
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
) -> Result<T, TassadarPreemptiveJobFairnessReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarPreemptiveJobFairnessReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPreemptiveJobFairnessReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarPreemptiveJobSchedulerStatus, build_tassadar_preemptive_job_fairness_report,
        read_json, tassadar_preemptive_job_fairness_report_path,
        write_tassadar_preemptive_job_fairness_report,
    };
    use tempfile::tempdir;

    #[test]
    fn preemptive_job_fairness_report_keeps_green_and_refused_schedulers_explicit() {
        let report = build_tassadar_preemptive_job_fairness_report();

        assert_eq!(report.green_scheduler_ids.len(), 2);
        assert_eq!(report.refused_scheduler_ids.len(), 1);
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.refusal_case_count, 2);
    }

    #[test]
    fn preemptive_job_fairness_report_tracks_fairness_rows() {
        let report = build_tassadar_preemptive_job_fairness_report();
        let row = report
            .rows
            .iter()
            .find(|row| row.scheduler_id == "deterministic_round_robin")
            .expect("green row");
        assert_eq!(row.status, TassadarPreemptiveJobSchedulerStatus::Green);
        assert!(row.starvation_free);
        let refused = report
            .rows
            .iter()
            .find(|row| row.scheduler_id == "host_nondeterministic_scheduler")
            .expect("refused row");
        assert_eq!(
            refused.status,
            TassadarPreemptiveJobSchedulerStatus::Refused
        );
    }

    #[test]
    fn preemptive_job_fairness_report_matches_committed_truth() {
        let generated = build_tassadar_preemptive_job_fairness_report();
        let committed =
            read_json(tassadar_preemptive_job_fairness_report_path()).expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_preemptive_job_fairness_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_preemptive_job_fairness_report.json");
        let report =
            write_tassadar_preemptive_job_fairness_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_preemptive_job_fairness_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_preemptive_job_fairness_report.json")
        );
    }
}
