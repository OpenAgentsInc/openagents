use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_THREADS_SCHEDULER_SANDBOX_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_threads_scheduler_sandbox_boundary_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarThreadsSchedulerBoundaryStatus {
    AllowedDeterministic,
    RefusedOutOfEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarThreadsSchedulerBoundaryRow {
    pub case_id: String,
    pub scheduler_id: String,
    pub shared_memory_shape_id: String,
    pub memory_order_id: String,
    pub status: TassadarThreadsSchedulerBoundaryStatus,
    pub exact_schedule_replay: bool,
    pub sandbox_descriptor_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarThreadsSchedulerSandboxBoundaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub allowed_scheduler_ids: Vec<String>,
    pub refused_scheduler_ids: Vec<String>,
    pub allowed_case_count: u32,
    pub refused_case_count: u32,
    pub rows: Vec<TassadarThreadsSchedulerBoundaryRow>,
    pub world_mount_dependency_marker: String,
    pub kernel_policy_dependency_marker: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarThreadsSchedulerSandboxBoundaryError {
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

#[must_use]
pub fn build_tassadar_threads_scheduler_sandbox_boundary_report()
-> TassadarThreadsSchedulerSandboxBoundaryReport {
    let rows = vec![
        row(
            "round_robin_shared_counter",
            "deterministic_round_robin_v1",
            "two_thread_shared_i32",
            "seq_cst",
            TassadarThreadsSchedulerBoundaryStatus::AllowedDeterministic,
            true,
            false,
            None,
            "deterministic round-robin shared-counter stepping is admitted only because schedule order and memory ordering are frozen explicitly",
        ),
        row(
            "barrier_then_reduce",
            "deterministic_barrier_release_v1",
            "two_thread_shared_slice_i32",
            "acquire_release",
            TassadarThreadsSchedulerBoundaryStatus::AllowedDeterministic,
            true,
            false,
            None,
            "deterministic barrier release plus reduce stays inside the research envelope when barrier order is fixed and replayable",
        ),
        row(
            "host_scheduler_refusal",
            "host_nondeterministic_runtime",
            "two_thread_shared_i32",
            "seq_cst",
            TassadarThreadsSchedulerBoundaryStatus::RefusedOutOfEnvelope,
            false,
            true,
            Some("nondeterministic_scheduler"),
            "host-owned scheduler selection remains refused because replay and challenge receipts would collapse without a frozen scheduler trace",
        ),
        row(
            "relaxed_memory_order_refusal",
            "deterministic_round_robin_v1",
            "two_thread_shared_i32",
            "relaxed",
            TassadarThreadsSchedulerBoundaryStatus::RefusedOutOfEnvelope,
            false,
            true,
            Some("unsupported_memory_ordering"),
            "relaxed memory ordering remains refused because the deterministic scheduler envelope does not rescue weak-order semantics",
        ),
    ];
    let allowed_scheduler_ids = rows
        .iter()
        .filter(|row| row.status == TassadarThreadsSchedulerBoundaryStatus::AllowedDeterministic)
        .map(|row| row.scheduler_id.clone())
        .collect::<Vec<_>>();
    let refused_scheduler_ids = rows
        .iter()
        .filter(|row| row.status == TassadarThreadsSchedulerBoundaryStatus::RefusedOutOfEnvelope)
        .map(|row| row.scheduler_id.clone())
        .collect::<Vec<_>>();
    let mut report = TassadarThreadsSchedulerSandboxBoundaryReport {
        schema_version: 1,
        report_id: String::from("tassadar.threads_scheduler_sandbox_boundary.report.v1"),
        profile_id: String::from("tassadar.research_profile.threads_deterministic_scheduler.v1"),
        allowed_case_count: allowed_scheduler_ids.len() as u32,
        refused_case_count: refused_scheduler_ids.len() as u32,
        allowed_scheduler_ids,
        refused_scheduler_ids,
        rows,
        world_mount_dependency_marker: String::from(
            "world-mounts remain the authority owner for task-scoped shared-memory and scheduler admission outside standalone psionic",
        ),
        kernel_policy_dependency_marker: String::from(
            "kernel-policy remains the authority owner for settlement-grade scheduler and side-effect admission outside standalone psionic",
        ),
        claim_boundary: String::from(
            "this sandbox boundary admits only named deterministic scheduler shapes for the research threads profile and refuses host-nondeterministic scheduling plus relaxed shared-memory ordering. It does not create a served threads lane, arbitrary shared-memory portability, or general concurrency closure",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Threads scheduler sandbox boundary report freezes {} allowed deterministic cases and {} refused out-of-envelope cases.",
        report.allowed_case_count, report.refused_case_count,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_threads_scheduler_sandbox_boundary_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_threads_scheduler_sandbox_boundary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_THREADS_SCHEDULER_SANDBOX_BOUNDARY_REPORT_REF)
}

pub fn write_tassadar_threads_scheduler_sandbox_boundary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarThreadsSchedulerSandboxBoundaryReport,
    TassadarThreadsSchedulerSandboxBoundaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarThreadsSchedulerSandboxBoundaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_threads_scheduler_sandbox_boundary_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarThreadsSchedulerSandboxBoundaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

#[cfg(test)]
pub fn load_tassadar_threads_scheduler_sandbox_boundary_report(
    path: impl AsRef<Path>,
) -> Result<
    TassadarThreadsSchedulerSandboxBoundaryReport,
    TassadarThreadsSchedulerSandboxBoundaryError,
> {
    read_json(path)
}

#[allow(clippy::too_many_arguments)]
fn row(
    case_id: &str,
    scheduler_id: &str,
    shared_memory_shape_id: &str,
    memory_order_id: &str,
    status: TassadarThreadsSchedulerBoundaryStatus,
    exact_schedule_replay: bool,
    sandbox_descriptor_required: bool,
    refusal_reason_id: Option<&str>,
    note: &str,
) -> TassadarThreadsSchedulerBoundaryRow {
    TassadarThreadsSchedulerBoundaryRow {
        case_id: String::from(case_id),
        scheduler_id: String::from(scheduler_id),
        shared_memory_shape_id: String::from(shared_memory_shape_id),
        memory_order_id: String::from(memory_order_id),
        status,
        exact_schedule_replay,
        sandbox_descriptor_required,
        refusal_reason_id: refusal_reason_id.map(String::from),
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

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarThreadsSchedulerSandboxBoundaryError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(|error| TassadarThreadsSchedulerSandboxBoundaryError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarThreadsSchedulerSandboxBoundaryError::Deserialize {
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
        TassadarThreadsSchedulerBoundaryStatus,
        build_tassadar_threads_scheduler_sandbox_boundary_report,
        load_tassadar_threads_scheduler_sandbox_boundary_report,
        tassadar_threads_scheduler_sandbox_boundary_report_path,
        write_tassadar_threads_scheduler_sandbox_boundary_report,
    };

    #[test]
    fn threads_scheduler_boundary_keeps_deterministic_and_refused_rows_explicit() {
        let report = build_tassadar_threads_scheduler_sandbox_boundary_report();

        assert_eq!(report.allowed_case_count, 2);
        assert_eq!(report.refused_case_count, 2);
        assert!(report.rows.iter().any(|row| {
            row.scheduler_id == "deterministic_round_robin_v1"
                && row.status == TassadarThreadsSchedulerBoundaryStatus::AllowedDeterministic
        }));
        assert!(
            report.rows.iter().any(|row| {
                row.refusal_reason_id.as_deref() == Some("nondeterministic_scheduler")
            })
        );
    }

    #[test]
    fn threads_scheduler_boundary_matches_committed_truth() {
        let generated = build_tassadar_threads_scheduler_sandbox_boundary_report();
        let committed = load_tassadar_threads_scheduler_sandbox_boundary_report(
            tassadar_threads_scheduler_sandbox_boundary_report_path(),
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_threads_scheduler_boundary_persists_current_truth() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_threads_scheduler_sandbox_boundary_report.json");
        let written = write_tassadar_threads_scheduler_sandbox_boundary_report(&output_path)
            .expect("write report");
        let persisted = load_tassadar_threads_scheduler_sandbox_boundary_report(&output_path)
            .expect("persisted report");
        assert_eq!(written, persisted);
    }
}
