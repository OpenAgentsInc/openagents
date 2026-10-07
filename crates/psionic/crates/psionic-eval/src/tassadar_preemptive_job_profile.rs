use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_cluster::{
    TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF, TassadarPreemptiveJobFairnessReport,
    build_tassadar_preemptive_job_fairness_report,
};
use psionic_runtime::{
    TASSADAR_PREEMPTIVE_JOB_BUNDLE_FILE, TASSADAR_PREEMPTIVE_JOB_PROFILE_ID,
    TASSADAR_PREEMPTIVE_JOB_RUN_ROOT_REF, TassadarCheckpointWorkloadFamily,
    TassadarPreemptiveJobRefusalKind, TassadarPreemptiveJobRuntimeBundle,
    TassadarPreemptiveJobStatus, build_tassadar_preemptive_job_runtime_bundle,
};

/// Stable committed report ref for the bounded preemptive-job profile.
pub const TASSADAR_PREEMPTIVE_JOB_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_preemptive_job_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveJobCaseAudit {
    pub case_id: String,
    pub job_id: String,
    pub process_id: String,
    pub workload_family: TassadarCheckpointWorkloadFamily,
    pub scheduler_id: String,
    pub runtime_status: TassadarPreemptiveJobStatus,
    pub exact_slice_boundary_parity: bool,
    pub exact_resume_parity: bool,
    pub starvation_free: bool,
    pub fairness_window_slices: u32,
    pub max_consecutive_wait_slices: u32,
    pub refusal_kinds: Vec<TassadarPreemptiveJobRefusalKind>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveJobProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarPreemptiveJobRuntimeBundle,
    pub fairness_report_ref: String,
    pub fairness_report: TassadarPreemptiveJobFairnessReport,
    pub case_audits: Vec<TassadarPreemptiveJobCaseAudit>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub overall_green: bool,
    pub green_scheduler_ids: Vec<String>,
    pub refused_scheduler_ids: Vec<String>,
    pub resumable_process_ids: Vec<String>,
    pub served_publication_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPreemptiveJobProfileReportError {
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

pub fn build_tassadar_preemptive_job_profile_report()
-> Result<TassadarPreemptiveJobProfileReport, TassadarPreemptiveJobProfileReportError> {
    let runtime_bundle = build_tassadar_preemptive_job_runtime_bundle();
    let fairness_report = build_tassadar_preemptive_job_fairness_report();
    let case_audits = runtime_bundle
        .case_receipts
        .iter()
        .map(|case| TassadarPreemptiveJobCaseAudit {
            case_id: case.case_id.clone(),
            job_id: case.job_id.clone(),
            process_id: case.process_id.clone(),
            workload_family: case.workload_family,
            scheduler_id: case.scheduler_id.clone(),
            runtime_status: case.status,
            exact_slice_boundary_parity: case.exact_slice_boundary_parity,
            exact_resume_parity: case.exact_resume_parity,
            starvation_free: case.starvation_free,
            fairness_window_slices: case.fairness_window_slices,
            max_consecutive_wait_slices: case.max_consecutive_wait_slices,
            refusal_kinds: case
                .refusal_cases
                .iter()
                .map(|refusal| refusal.refusal_kind)
                .collect(),
            note: case.note.clone(),
        })
        .collect::<Vec<_>>();
    let exact_case_count = case_audits
        .iter()
        .filter(|case| case.runtime_status == TassadarPreemptiveJobStatus::ExactSliceBoundaryParity)
        .count() as u32;
    let refusal_case_count = case_audits
        .iter()
        .map(|case| case.refusal_kinds.len() as u32)
        .sum();
    let resumable_process_ids = case_audits
        .iter()
        .filter(|case| case.runtime_status == TassadarPreemptiveJobStatus::ExactSliceBoundaryParity)
        .map(|case| case.process_id.clone())
        .collect::<Vec<_>>();
    let overall_green = exact_case_count == runtime_bundle.exact_case_count
        && refusal_case_count == runtime_bundle.refusal_case_count
        && fairness_report.exact_case_count == runtime_bundle.exact_case_count
        && fairness_report.refusal_case_count == runtime_bundle.refusal_case_count
        && fairness_report.fairness_window_green_case_count == runtime_bundle.exact_case_count
        && !fairness_report.green_scheduler_ids.is_empty()
        && !fairness_report.refused_scheduler_ids.is_empty();
    let mut report = TassadarPreemptiveJobProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.preemptive_job_profile.report.v1"),
        profile_id: String::from(TASSADAR_PREEMPTIVE_JOB_PROFILE_ID),
        runtime_bundle_ref: format!(
            "{}/{}",
            TASSADAR_PREEMPTIVE_JOB_RUN_ROOT_REF, TASSADAR_PREEMPTIVE_JOB_BUNDLE_FILE
        ),
        runtime_bundle,
        fairness_report_ref: String::from(TASSADAR_PREEMPTIVE_JOB_FAIRNESS_REPORT_REF),
        fairness_report,
        case_audits,
        exact_case_count,
        refusal_case_count,
        overall_green,
        green_scheduler_ids: Vec::new(),
        refused_scheduler_ids: Vec::new(),
        resumable_process_ids,
        served_publication_allowed: false,
        claim_boundary: String::from(
            "this eval report covers one bounded preemptive-job profile with deterministic scheduler envelopes, explicit slice-boundary receipts, resumable checkpoints, and operator-visible fairness truth. It does not claim arbitrary host scheduling, general concurrency closure, or served public widening for preemptive jobs",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.green_scheduler_ids = report.fairness_report.green_scheduler_ids.clone();
    report.refused_scheduler_ids = report.fairness_report.refused_scheduler_ids.clone();
    report.summary = format!(
        "Preemptive-job profile report covers exact_cases={}, refusal_rows={}, green_schedulers={}, refused_schedulers={}, served_publication_allowed={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.green_scheduler_ids.len(),
        report.refused_scheduler_ids.len(),
        report.served_publication_allowed,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_preemptive_job_profile_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_preemptive_job_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_PREEMPTIVE_JOB_PROFILE_REPORT_REF)
}

pub fn write_tassadar_preemptive_job_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPreemptiveJobProfileReport, TassadarPreemptiveJobProfileReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPreemptiveJobProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_preemptive_job_profile_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPreemptiveJobProfileReportError::Write {
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
) -> Result<T, TassadarPreemptiveJobProfileReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarPreemptiveJobProfileReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPreemptiveJobProfileReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_preemptive_job_profile_report, read_json,
        tassadar_preemptive_job_profile_report_path, write_tassadar_preemptive_job_profile_report,
    };
    use tempfile::tempdir;

    #[test]
    fn preemptive_job_profile_report_keeps_scheduler_truth_explicit() {
        let report = build_tassadar_preemptive_job_profile_report().expect("report");

        assert!(report.overall_green);
        assert_eq!(
            report.profile_id,
            "tassadar.internal_compute.preemptive_jobs.v1"
        );
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.refusal_case_count, 2);
        assert_eq!(report.green_scheduler_ids.len(), 2);
        assert_eq!(
            report.refused_scheduler_ids,
            vec![String::from("host_nondeterministic_scheduler")]
        );
        assert!(!report.served_publication_allowed);
    }

    #[test]
    fn preemptive_job_profile_report_keeps_resumable_processes_explicit() {
        let report = build_tassadar_preemptive_job_profile_report().expect("report");

        assert!(
            report
                .resumable_process_ids
                .contains(&String::from("tassadar.process.long_loop_kernel.v1"))
        );
        assert!(
            report
                .resumable_process_ids
                .contains(&String::from("tassadar.process.search_frontier_kernel.v1"))
        );
    }

    #[test]
    fn preemptive_job_profile_report_matches_committed_truth() {
        let generated = build_tassadar_preemptive_job_profile_report().expect("report");
        let committed =
            read_json(tassadar_preemptive_job_profile_report_path()).expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_preemptive_job_profile_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir.path().join("tassadar_preemptive_job_report.json");
        let report =
            write_tassadar_preemptive_job_profile_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_preemptive_job_profile_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_preemptive_job_report.json")
        );
    }
}
