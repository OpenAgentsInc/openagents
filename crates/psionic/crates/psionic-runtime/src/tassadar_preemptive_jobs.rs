use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::TassadarCheckpointWorkloadFamily;

/// Stable post-process profile for bounded preemptive jobs.
pub const TASSADAR_PREEMPTIVE_JOB_PROFILE_ID: &str = "tassadar.internal_compute.preemptive_jobs.v1";
/// Stable run root for the committed preemptive-job bundle.
pub const TASSADAR_PREEMPTIVE_JOB_RUN_ROOT_REF: &str =
    "fixtures/tassadar/runs/tassadar_preemptive_jobs_v1";
/// Stable runtime-bundle filename under the committed run root.
pub const TASSADAR_PREEMPTIVE_JOB_BUNDLE_FILE: &str = "tassadar_preemptive_job_runtime_bundle.json";

/// Runtime status for one bounded preemptive job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPreemptiveJobStatus {
    ExactSliceBoundaryParity,
    ExactRefusalParity,
}

/// Typed refusal for unsupported scheduler regimes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPreemptiveJobRefusalKind {
    UnsupportedSchedulerRegime,
    FairnessRegimeOutOfEnvelope,
}

/// Receipt for one preempted slice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveSliceReceipt {
    pub slice_id: String,
    pub slice_index: u32,
    pub scheduler_id: String,
    pub executed_step_count: u32,
    pub cumulative_step_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resumed_from_checkpoint_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emitted_checkpoint_id: Option<String>,
    pub wait_slices_before_run: u32,
    pub preempted_after_slice: bool,
    pub detail: String,
}

/// One explicit refusal attached to a bounded preemptive job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveJobRefusal {
    pub refusal_kind: TassadarPreemptiveJobRefusalKind,
    pub scheduler_id: String,
    pub detail: String,
}

/// Runtime receipt for one bounded preemptive job.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveJobReceipt {
    pub case_id: String,
    pub job_id: String,
    pub process_id: String,
    pub workload_family: TassadarCheckpointWorkloadFamily,
    pub scheduler_id: String,
    pub profile_id: String,
    pub slice_budget_steps: u32,
    pub fairness_window_slices: u32,
    pub max_consecutive_wait_slices: u32,
    pub preemption_count: u32,
    pub starvation_free: bool,
    pub exact_slice_boundary_parity: bool,
    pub exact_resume_parity: bool,
    pub status: TassadarPreemptiveJobStatus,
    pub slice_receipts: Vec<TassadarPreemptiveSliceReceipt>,
    pub refusal_cases: Vec<TassadarPreemptiveJobRefusal>,
    pub note: String,
    pub receipt_digest: String,
}

/// Canonical runtime bundle for the bounded preemptive-job lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPreemptiveJobRuntimeBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub profile_id: String,
    pub case_receipts: Vec<TassadarPreemptiveJobReceipt>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub green_scheduler_ids: Vec<String>,
    pub refused_scheduler_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPreemptiveJobRuntimeBundleError {
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
pub fn build_tassadar_preemptive_job_runtime_bundle() -> TassadarPreemptiveJobRuntimeBundle {
    let case_receipts = vec![
        round_robin_counter_job(),
        weighted_fair_search_job(),
        host_scheduler_refusal_job(),
    ];
    let exact_case_count = case_receipts
        .iter()
        .filter(|case| case.status == TassadarPreemptiveJobStatus::ExactSliceBoundaryParity)
        .count() as u32;
    let refusal_case_count = case_receipts
        .iter()
        .map(|case| case.refusal_cases.len() as u32)
        .sum();
    let mut green_scheduler_ids = case_receipts
        .iter()
        .filter(|case| case.status == TassadarPreemptiveJobStatus::ExactSliceBoundaryParity)
        .map(|case| case.scheduler_id.clone())
        .collect::<Vec<_>>();
    green_scheduler_ids.sort();
    green_scheduler_ids.dedup();
    let mut refused_scheduler_ids = case_receipts
        .iter()
        .filter(|case| case.status == TassadarPreemptiveJobStatus::ExactRefusalParity)
        .map(|case| case.scheduler_id.clone())
        .collect::<Vec<_>>();
    refused_scheduler_ids.sort();
    refused_scheduler_ids.dedup();
    let mut bundle = TassadarPreemptiveJobRuntimeBundle {
        schema_version: 1,
        bundle_id: String::from("tassadar.preemptive_job.runtime_bundle.v1"),
        profile_id: String::from(TASSADAR_PREEMPTIVE_JOB_PROFILE_ID),
        case_receipts,
        exact_case_count,
        refusal_case_count,
        green_scheduler_ids,
        refused_scheduler_ids,
        claim_boundary: String::from(
            "this runtime bundle freezes one bounded preemptive-job lane with deterministic slice boundaries, resumable checkpoints, and explicit scheduler-regime refusal truth. It does not claim arbitrary host scheduling, general concurrency closure, non-deterministic fairness, or broader served internal compute",
        ),
        summary: String::new(),
        bundle_digest: String::new(),
    };
    bundle.summary = format!(
        "Preemptive-job runtime bundle covers exact_cases={}, refusal_rows={}, green_schedulers={}, refused_schedulers={}.",
        bundle.exact_case_count,
        bundle.refusal_case_count,
        bundle.green_scheduler_ids.len(),
        bundle.refused_scheduler_ids.len(),
    );
    bundle.bundle_digest =
        stable_digest(b"psionic_tassadar_preemptive_job_runtime_bundle|", &bundle);
    bundle
}

#[must_use]
pub fn tassadar_preemptive_job_runtime_bundle_path() -> PathBuf {
    repo_root()
        .join(TASSADAR_PREEMPTIVE_JOB_RUN_ROOT_REF)
        .join(TASSADAR_PREEMPTIVE_JOB_BUNDLE_FILE)
}

pub fn write_tassadar_preemptive_job_runtime_bundle(
    output_path: impl AsRef<Path>,
) -> Result<TassadarPreemptiveJobRuntimeBundle, TassadarPreemptiveJobRuntimeBundleError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPreemptiveJobRuntimeBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = build_tassadar_preemptive_job_runtime_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPreemptiveJobRuntimeBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

fn round_robin_counter_job() -> TassadarPreemptiveJobReceipt {
    let slices = vec![
        slice(
            "counter_round_robin_job",
            0,
            "deterministic_round_robin",
            64,
            64,
            None,
            Some("checkpoint.counter_round_robin.00"),
            0,
            true,
        ),
        slice(
            "counter_round_robin_job",
            1,
            "deterministic_round_robin",
            64,
            128,
            Some("checkpoint.counter_round_robin.00"),
            Some("checkpoint.counter_round_robin.01"),
            1,
            true,
        ),
        slice(
            "counter_round_robin_job",
            2,
            "deterministic_round_robin",
            48,
            176,
            Some("checkpoint.counter_round_robin.01"),
            None,
            1,
            false,
        ),
    ];
    receipt(
        "counter_round_robin_job",
        "tassadar.job.counter_round_robin.v1",
        "tassadar.process.long_loop_kernel.v1",
        TassadarCheckpointWorkloadFamily::LongLoopKernel,
        "deterministic_round_robin",
        64,
        2,
        1,
        2,
        true,
        true,
        true,
        TassadarPreemptiveJobStatus::ExactSliceBoundaryParity,
        slices,
        Vec::new(),
        "round-robin slice boundaries preserve exact resume parity and keep bounded wait at one slice",
    )
}

fn weighted_fair_search_job() -> TassadarPreemptiveJobReceipt {
    let slices = vec![
        slice(
            "search_weighted_fair_job",
            0,
            "weighted_fair_slice_rotation",
            40,
            40,
            None,
            Some("checkpoint.search_weighted_fair.00"),
            0,
            true,
        ),
        slice(
            "search_weighted_fair_job",
            1,
            "weighted_fair_slice_rotation",
            40,
            80,
            Some("checkpoint.search_weighted_fair.00"),
            Some("checkpoint.search_weighted_fair.01"),
            2,
            true,
        ),
        slice(
            "search_weighted_fair_job",
            2,
            "weighted_fair_slice_rotation",
            40,
            120,
            Some("checkpoint.search_weighted_fair.01"),
            Some("checkpoint.search_weighted_fair.02"),
            1,
            true,
        ),
        slice(
            "search_weighted_fair_job",
            3,
            "weighted_fair_slice_rotation",
            24,
            144,
            Some("checkpoint.search_weighted_fair.02"),
            None,
            2,
            false,
        ),
    ];
    receipt(
        "search_weighted_fair_job",
        "tassadar.job.search_weighted_fair.v1",
        "tassadar.process.search_frontier_kernel.v1",
        TassadarCheckpointWorkloadFamily::SearchFrontierKernel,
        "weighted_fair_slice_rotation",
        40,
        3,
        2,
        3,
        true,
        true,
        true,
        TassadarPreemptiveJobStatus::ExactSliceBoundaryParity,
        slices,
        Vec::new(),
        "weighted fair rotation preserves exact slice-boundary parity while bounding starvation to two wait slices",
    )
}

fn host_scheduler_refusal_job() -> TassadarPreemptiveJobReceipt {
    receipt(
        "host_scheduler_nondeterministic_refusal",
        "tassadar.job.host_scheduler_nondeterministic.v1",
        "tassadar.process.state_machine_accumulator.v1",
        TassadarCheckpointWorkloadFamily::StateMachineAccumulator,
        "host_nondeterministic_scheduler",
        64,
        0,
        0,
        0,
        false,
        false,
        false,
        TassadarPreemptiveJobStatus::ExactRefusalParity,
        Vec::new(),
        vec![
            TassadarPreemptiveJobRefusal {
                refusal_kind: TassadarPreemptiveJobRefusalKind::UnsupportedSchedulerRegime,
                scheduler_id: String::from("host_nondeterministic_scheduler"),
                detail: String::from(
                    "host nondeterministic scheduling stays outside the bounded preemptive-job lane",
                ),
            },
            TassadarPreemptiveJobRefusal {
                refusal_kind: TassadarPreemptiveJobRefusalKind::FairnessRegimeOutOfEnvelope,
                scheduler_id: String::from("host_nondeterministic_scheduler"),
                detail: String::from(
                    "unbounded or unverified fairness regimes remain explicit refusals instead of implicit operator trust",
                ),
            },
        ],
        "host nondeterministic scheduling remains on an explicit refusal path instead of widening the bounded preemptive-job claim",
    )
}

fn receipt(
    case_id: &str,
    job_id: &str,
    process_id: &str,
    workload_family: TassadarCheckpointWorkloadFamily,
    scheduler_id: &str,
    slice_budget_steps: u32,
    fairness_window_slices: u32,
    max_consecutive_wait_slices: u32,
    preemption_count: u32,
    starvation_free: bool,
    exact_slice_boundary_parity: bool,
    exact_resume_parity: bool,
    status: TassadarPreemptiveJobStatus,
    slice_receipts: Vec<TassadarPreemptiveSliceReceipt>,
    refusal_cases: Vec<TassadarPreemptiveJobRefusal>,
    note: &str,
) -> TassadarPreemptiveJobReceipt {
    let mut receipt = TassadarPreemptiveJobReceipt {
        case_id: String::from(case_id),
        job_id: String::from(job_id),
        process_id: String::from(process_id),
        workload_family,
        scheduler_id: String::from(scheduler_id),
        profile_id: String::from(TASSADAR_PREEMPTIVE_JOB_PROFILE_ID),
        slice_budget_steps,
        fairness_window_slices,
        max_consecutive_wait_slices,
        preemption_count,
        starvation_free,
        exact_slice_boundary_parity,
        exact_resume_parity,
        status,
        slice_receipts,
        refusal_cases,
        note: String::from(note),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_digest(b"psionic_tassadar_preemptive_job_receipt|", &receipt);
    receipt
}

fn slice(
    case_id: &str,
    slice_index: u32,
    scheduler_id: &str,
    executed_step_count: u32,
    cumulative_step_count: u32,
    resumed_from_checkpoint_id: Option<&str>,
    emitted_checkpoint_id: Option<&str>,
    wait_slices_before_run: u32,
    preempted_after_slice: bool,
) -> TassadarPreemptiveSliceReceipt {
    TassadarPreemptiveSliceReceipt {
        slice_id: format!("{case_id}::slice::{slice_index:02}"),
        slice_index,
        scheduler_id: String::from(scheduler_id),
        executed_step_count,
        cumulative_step_count,
        resumed_from_checkpoint_id: resumed_from_checkpoint_id.map(String::from),
        emitted_checkpoint_id: emitted_checkpoint_id.map(String::from),
        wait_slices_before_run,
        preempted_after_slice,
        detail: format!(
            "slice {slice_index} under `{scheduler_id}` executed {executed_step_count} steps after waiting {wait_slices_before_run} slice(s)"
        ),
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
) -> Result<T, TassadarPreemptiveJobRuntimeBundleError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarPreemptiveJobRuntimeBundleError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPreemptiveJobRuntimeBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_PREEMPTIVE_JOB_PROFILE_ID, TassadarPreemptiveJobStatus,
        build_tassadar_preemptive_job_runtime_bundle, read_json,
        tassadar_preemptive_job_runtime_bundle_path, write_tassadar_preemptive_job_runtime_bundle,
    };
    use tempfile::tempdir;

    #[test]
    fn preemptive_job_runtime_bundle_keeps_operator_truth_explicit() {
        let bundle = build_tassadar_preemptive_job_runtime_bundle();

        assert_eq!(bundle.profile_id, TASSADAR_PREEMPTIVE_JOB_PROFILE_ID);
        assert_eq!(bundle.exact_case_count, 2);
        assert_eq!(bundle.refusal_case_count, 2);
        assert_eq!(
            bundle.green_scheduler_ids,
            vec![
                String::from("deterministic_round_robin"),
                String::from("weighted_fair_slice_rotation")
            ]
        );
        assert_eq!(
            bundle.refused_scheduler_ids,
            vec![String::from("host_nondeterministic_scheduler")]
        );
    }

    #[test]
    fn preemptive_job_runtime_bundle_tracks_exact_and_refusal_rows() {
        let bundle = build_tassadar_preemptive_job_runtime_bundle();
        let exact_case = bundle
            .case_receipts
            .iter()
            .find(|case| case.case_id == "counter_round_robin_job")
            .expect("exact case");
        assert_eq!(
            exact_case.status,
            TassadarPreemptiveJobStatus::ExactSliceBoundaryParity
        );
        assert!(exact_case.exact_resume_parity);
        assert_eq!(exact_case.max_consecutive_wait_slices, 1);
        let refusal_case = bundle
            .case_receipts
            .iter()
            .find(|case| case.case_id == "host_scheduler_nondeterministic_refusal")
            .expect("refusal case");
        assert_eq!(
            refusal_case.status,
            TassadarPreemptiveJobStatus::ExactRefusalParity
        );
        assert!(!refusal_case.starvation_free);
        assert_eq!(refusal_case.refusal_cases.len(), 2);
    }

    #[test]
    fn preemptive_job_runtime_bundle_matches_committed_truth() {
        let generated = build_tassadar_preemptive_job_runtime_bundle();
        let committed =
            read_json(tassadar_preemptive_job_runtime_bundle_path()).expect("committed bundle");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_preemptive_job_runtime_bundle_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_preemptive_job_runtime_bundle.json");
        let bundle =
            write_tassadar_preemptive_job_runtime_bundle(&output_path).expect("write bundle");
        let persisted = read_json(&output_path).expect("persisted bundle");

        assert_eq!(bundle, persisted);
        assert_eq!(
            tassadar_preemptive_job_runtime_bundle_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_preemptive_job_runtime_bundle.json")
        );
    }
}
