use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID: &str =
    "tassadar.internal_compute.async_lifecycle.v1";
pub const TASSADAR_ASYNC_LIFECYCLE_PROFILE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_async_lifecycle_profile_runtime_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAsyncLifecycleEventKind {
    Launch,
    InterruptRequested,
    CheckpointPersisted,
    RetryScheduled,
    Resumed,
    CancelAcknowledged,
    Completed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAsyncLifecycleEvent {
    pub event_id: String,
    pub order_index: u32,
    pub kind: TassadarAsyncLifecycleEventKind,
    pub boundary_id: String,
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarAsyncLifecycleCaseStatus {
    ExactDeterministicParity,
    ExactRefusalParity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAsyncLifecycleCaseRow {
    pub case_id: String,
    pub process_id: String,
    pub lifecycle_surface_id: String,
    pub state_shape_id: String,
    pub route_profile_id: String,
    pub events: Vec<TassadarAsyncLifecycleEvent>,
    pub status: TassadarAsyncLifecycleCaseStatus,
    pub exact_interrupt_parity: bool,
    pub exact_retry_parity: bool,
    pub exact_cancellation_parity: bool,
    pub retry_attempt_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub final_state_digest: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarAsyncLifecycleProfileRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub rows: Vec<TassadarAsyncLifecycleCaseRow>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub overall_green: bool,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub routeable_lifecycle_surface_ids: Vec<String>,
    pub refused_lifecycle_surface_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarAsyncLifecycleProfileRuntimeReportError {
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
pub fn build_tassadar_async_lifecycle_profile_runtime_report(
) -> TassadarAsyncLifecycleProfileRuntimeReport {
    let rows = vec![
        exact_row(
            "interrupt_resume_counter_job",
            "tassadar.process.long_loop_kernel.v1",
            "interruptible_counter_job",
            "counter_register_and_checkpoint_cursor",
            &[
                (TassadarAsyncLifecycleEventKind::Launch, "launch.boundary"),
                (
                    TassadarAsyncLifecycleEventKind::InterruptRequested,
                    "interrupt.boundary",
                ),
                (
                    TassadarAsyncLifecycleEventKind::CheckpointPersisted,
                    "checkpoint.async_interrupt.00",
                ),
                (
                    TassadarAsyncLifecycleEventKind::Resumed,
                    "resume.async_interrupt.00",
                ),
                (TassadarAsyncLifecycleEventKind::Completed, "complete.boundary"),
            ],
            true,
            false,
            false,
            0,
            "interrupt-and-resume stays exact because the interrupt boundary and resume checkpoint remain deterministic and explicit",
        ),
        exact_row(
            "retryable_timeout_search_job",
            "tassadar.process.search_frontier_kernel.v1",
            "retryable_timeout_search_job",
            "search_frontier_and_retry_budget",
            &[
                (TassadarAsyncLifecycleEventKind::Launch, "launch.boundary"),
                (
                    TassadarAsyncLifecycleEventKind::CheckpointPersisted,
                    "checkpoint.retryable_timeout.00",
                ),
                (
                    TassadarAsyncLifecycleEventKind::RetryScheduled,
                    "retry_budget.attempt_1",
                ),
                (
                    TassadarAsyncLifecycleEventKind::Resumed,
                    "resume.retryable_timeout.01",
                ),
                (TassadarAsyncLifecycleEventKind::Completed, "complete.boundary"),
            ],
            false,
            true,
            false,
            1,
            "bounded retry stays exact because the retry budget, checkpoint boundary, and resumed continuation are frozen explicitly",
        ),
        exact_row(
            "safe_boundary_cancellation_job",
            "tassadar.process.state_machine_accumulator.v1",
            "safe_boundary_cancellation_job",
            "state_machine_cursor_and_cancel_token",
            &[
                (TassadarAsyncLifecycleEventKind::Launch, "launch.boundary"),
                (
                    TassadarAsyncLifecycleEventKind::InterruptRequested,
                    "cancel.signal",
                ),
                (
                    TassadarAsyncLifecycleEventKind::CheckpointPersisted,
                    "checkpoint.safe_cancel.00",
                ),
                (
                    TassadarAsyncLifecycleEventKind::CancelAcknowledged,
                    "cancel.safe_boundary",
                ),
            ],
            false,
            false,
            true,
            0,
            "safe-boundary cancellation stays exact because acknowledgement happens only after an explicit checkpoint and replay-safe boundary",
        ),
        refusal_row(
            "open_ended_external_callback",
            "tassadar.process.external_callback.v1",
            "open_ended_external_callback",
            "ambient_callback_mailbox",
            "async_callback_out_of_envelope",
            &[
                (TassadarAsyncLifecycleEventKind::Launch, "launch.boundary"),
                (
                    TassadarAsyncLifecycleEventKind::InterruptRequested,
                    "ambient.callback.wait",
                ),
            ],
            "open-ended external callbacks stay outside the bounded async-lifecycle profile",
        ),
        refusal_row(
            "mid_effect_cancellation",
            "tassadar.process.effectful_mid_commit.v1",
            "mid_effect_cancellation",
            "effect_commit_cursor",
            "mid_effect_cancellation_unsafe",
            &[
                (TassadarAsyncLifecycleEventKind::Launch, "launch.boundary"),
                (
                    TassadarAsyncLifecycleEventKind::InterruptRequested,
                    "cancel.unsafe_mid_effect",
                ),
            ],
            "mid-effect cancellation stays refused because it does not preserve a replay-safe checkpoint boundary",
        ),
        refusal_row(
            "unbounded_retry_backoff",
            "tassadar.process.unbounded_retry.v1",
            "unbounded_retry_backoff",
            "retry_budget_unbounded",
            "unbounded_retry_policy_out_of_envelope",
            &[
                (TassadarAsyncLifecycleEventKind::Launch, "launch.boundary"),
                (
                    TassadarAsyncLifecycleEventKind::RetryScheduled,
                    "retry_budget.unbounded",
                ),
            ],
            "unbounded retry stays refused because the bounded async-lifecycle profile requires an explicit finite retry budget",
        ),
    ];
    let exact_case_count = rows
        .iter()
        .filter(|row| row.status == TassadarAsyncLifecycleCaseStatus::ExactDeterministicParity)
        .count() as u32;
    let refusal_case_count = rows
        .iter()
        .filter(|row| row.status == TassadarAsyncLifecycleCaseStatus::ExactRefusalParity)
        .count() as u32;
    let routeable_lifecycle_surface_ids = rows
        .iter()
        .filter(|row| row.status == TassadarAsyncLifecycleCaseStatus::ExactDeterministicParity)
        .map(|row| row.lifecycle_surface_id.clone())
        .collect::<Vec<_>>();
    let refused_lifecycle_surface_ids = rows
        .iter()
        .filter(|row| row.status == TassadarAsyncLifecycleCaseStatus::ExactRefusalParity)
        .map(|row| row.lifecycle_surface_id.clone())
        .collect::<Vec<_>>();
    let overall_green = exact_case_count >= 3
        && refusal_case_count >= 3
        && !routeable_lifecycle_surface_ids.is_empty();
    let mut report = TassadarAsyncLifecycleProfileRuntimeReport {
        schema_version: 1,
        report_id: String::from("tassadar.async_lifecycle_profile.runtime_report.v1"),
        profile_id: String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID),
        rows,
        exact_case_count,
        refusal_case_count,
        overall_green,
        public_profile_allowed_profile_ids: if overall_green {
            vec![String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID)]
        } else {
            Vec::new()
        },
        default_served_profile_allowed_profile_ids: Vec::new(),
        routeable_lifecycle_surface_ids,
        refused_lifecycle_surface_ids,
        claim_boundary: String::from(
            "this runtime report covers one bounded async-lifecycle profile with deterministic interrupt, bounded retry, and safe-boundary cancellation semantics. It keeps open-ended external callbacks, mid-effect cancellation, and unbounded retry policies on explicit refusal paths instead of implying generic async execution, arbitrary external events, or broader served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Async-lifecycle runtime report covers exact_cases={}, refusal_cases={}, routeable_surfaces={}, refused_surfaces={}, overall_green={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.routeable_lifecycle_surface_ids.len(),
        report.refused_lifecycle_surface_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_async_lifecycle_profile_runtime_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_async_lifecycle_profile_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ASYNC_LIFECYCLE_PROFILE_RUNTIME_REPORT_REF)
}

pub fn write_tassadar_async_lifecycle_profile_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarAsyncLifecycleProfileRuntimeReport,
    TassadarAsyncLifecycleProfileRuntimeReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarAsyncLifecycleProfileRuntimeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_async_lifecycle_profile_runtime_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarAsyncLifecycleProfileRuntimeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn exact_row(
    case_id: &str,
    process_id: &str,
    lifecycle_surface_id: &str,
    state_shape_id: &str,
    events: &[(TassadarAsyncLifecycleEventKind, &str)],
    exact_interrupt_parity: bool,
    exact_retry_parity: bool,
    exact_cancellation_parity: bool,
    retry_attempt_count: u32,
    note: &str,
) -> TassadarAsyncLifecycleCaseRow {
    TassadarAsyncLifecycleCaseRow {
        case_id: String::from(case_id),
        process_id: String::from(process_id),
        lifecycle_surface_id: String::from(lifecycle_surface_id),
        state_shape_id: String::from(state_shape_id),
        route_profile_id: String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID),
        events: events
            .iter()
            .enumerate()
            .map(|(order_index, (kind, boundary_id))| {
                event(case_id, order_index as u32, *kind, boundary_id)
            })
            .collect(),
        status: TassadarAsyncLifecycleCaseStatus::ExactDeterministicParity,
        exact_interrupt_parity,
        exact_retry_parity,
        exact_cancellation_parity,
        retry_attempt_count,
        refusal_reason_id: None,
        final_state_digest: stable_digest(
            b"psionic_tassadar_async_lifecycle_state|",
            &(case_id, lifecycle_surface_id, retry_attempt_count),
        ),
        note: String::from(note),
    }
}

fn refusal_row(
    case_id: &str,
    process_id: &str,
    lifecycle_surface_id: &str,
    state_shape_id: &str,
    refusal_reason_id: &str,
    events: &[(TassadarAsyncLifecycleEventKind, &str)],
    note: &str,
) -> TassadarAsyncLifecycleCaseRow {
    TassadarAsyncLifecycleCaseRow {
        case_id: String::from(case_id),
        process_id: String::from(process_id),
        lifecycle_surface_id: String::from(lifecycle_surface_id),
        state_shape_id: String::from(state_shape_id),
        route_profile_id: String::from(TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID),
        events: events
            .iter()
            .enumerate()
            .map(|(order_index, (kind, boundary_id))| {
                event(case_id, order_index as u32, *kind, boundary_id)
            })
            .collect(),
        status: TassadarAsyncLifecycleCaseStatus::ExactRefusalParity,
        exact_interrupt_parity: false,
        exact_retry_parity: false,
        exact_cancellation_parity: false,
        retry_attempt_count: 0,
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        final_state_digest: stable_digest(
            b"psionic_tassadar_async_lifecycle_refusal_state|",
            &(case_id, lifecycle_surface_id, refusal_reason_id),
        ),
        note: String::from(note),
    }
}

fn event(
    case_id: &str,
    order_index: u32,
    kind: TassadarAsyncLifecycleEventKind,
    boundary_id: &str,
) -> TassadarAsyncLifecycleEvent {
    TassadarAsyncLifecycleEvent {
        event_id: format!("{case_id}.event.{order_index}"),
        order_index,
        kind,
        boundary_id: String::from(boundary_id),
        note: format!("async lifecycle event `{kind:?}` at boundary `{boundary_id}`")
            .to_lowercase(),
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
) -> Result<T, TassadarAsyncLifecycleProfileRuntimeReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarAsyncLifecycleProfileRuntimeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarAsyncLifecycleProfileRuntimeReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID, build_tassadar_async_lifecycle_profile_runtime_report,
        read_json, tassadar_async_lifecycle_profile_runtime_report_path,
        write_tassadar_async_lifecycle_profile_runtime_report,
    };
    use tempfile::tempdir;

    #[test]
    fn async_lifecycle_runtime_report_keeps_interrupt_retry_and_cancel_explicit() {
        let report = build_tassadar_async_lifecycle_profile_runtime_report();

        assert!(report.overall_green);
        assert_eq!(report.profile_id, TASSADAR_ASYNC_LIFECYCLE_PROFILE_ID);
        assert_eq!(report.exact_case_count, 3);
        assert_eq!(report.refusal_case_count, 3);
        assert_eq!(report.routeable_lifecycle_surface_ids.len(), 3);
        assert_eq!(report.refused_lifecycle_surface_ids.len(), 3);
    }

    #[test]
    fn async_lifecycle_runtime_report_keeps_routeability_bounded() {
        let report = build_tassadar_async_lifecycle_profile_runtime_report();

        assert!(report
            .routeable_lifecycle_surface_ids
            .contains(&String::from("interruptible_counter_job")));
        assert!(report
            .routeable_lifecycle_surface_ids
            .contains(&String::from("retryable_timeout_search_job")));
        assert!(report
            .routeable_lifecycle_surface_ids
            .contains(&String::from("safe_boundary_cancellation_job")));
        assert!(report
            .refused_lifecycle_surface_ids
            .contains(&String::from("mid_effect_cancellation")));
    }

    #[test]
    fn async_lifecycle_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_async_lifecycle_profile_runtime_report();
        let committed = read_json(tassadar_async_lifecycle_profile_runtime_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_async_lifecycle_runtime_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_async_lifecycle_profile_runtime_report.json");
        let report = write_tassadar_async_lifecycle_profile_runtime_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_async_lifecycle_profile_runtime_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_async_lifecycle_profile_runtime_report.json")
        );
    }
}
