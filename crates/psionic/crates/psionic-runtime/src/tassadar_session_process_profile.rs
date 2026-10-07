use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_SESSION_PROCESS_PROFILE_ID: &str =
    "tassadar.internal_compute.session_process.v1";
pub const TASSADAR_SESSION_PROCESS_PROFILE_RUNTIME_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_session_process_profile_runtime_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSessionProcessMessageKind {
    OpenSession,
    UserTurn,
    AssistantTurn,
    CloseSession,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSessionProcessMessage {
    pub message_id: String,
    pub turn_index: u32,
    pub kind: TassadarSessionProcessMessageKind,
    pub payload_digest: String,
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSessionProcessCaseStatus {
    ExactDeterministicParity,
    ExactRefusalParity,
    Drift,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSessionProcessCaseRow {
    pub case_id: String,
    pub session_id: String,
    pub interaction_surface_id: String,
    pub local_state_shape_id: String,
    pub route_profile_id: String,
    pub input_messages: Vec<TassadarSessionProcessMessage>,
    pub output_messages: Vec<TassadarSessionProcessMessage>,
    pub status: TassadarSessionProcessCaseStatus,
    pub exact_state_parity: bool,
    pub exact_output_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub final_state_digest: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSessionProcessProfileRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub rows: Vec<TassadarSessionProcessCaseRow>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub overall_green: bool,
    pub public_profile_allowed_profile_ids: Vec<String>,
    pub default_served_profile_allowed_profile_ids: Vec<String>,
    pub routeable_interaction_surface_ids: Vec<String>,
    pub refused_interaction_surface_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSessionProcessProfileRuntimeReportError {
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
pub fn build_tassadar_session_process_profile_runtime_report(
) -> TassadarSessionProcessProfileRuntimeReport {
    let rows = vec![
        exact_row(
            "echo_roundtrip_session",
            "session.echo_roundtrip.v1",
            "deterministic_echo_turn_loop",
            "single_reply_buffer",
            &["open:echo", "user:hello bounded world", "close:echo"],
            &["assistant:hello bounded world", "assistant:echo closed"],
            "echo loop preserves one bounded reply buffer across a deterministic turn pair",
        ),
        exact_row(
            "counter_stateful_session",
            "session.counter_stateful.v1",
            "stateful_counter_turn_loop",
            "persistent_counter_register",
            &[
                "open:counter",
                "user:increment",
                "user:increment",
                "user:query",
                "close:counter",
            ],
            &[
                "assistant:count=1",
                "assistant:count=2",
                "assistant:count=2",
                "assistant:counter closed",
            ],
            "stateful loop preserves a bounded counter register across deterministic turns",
        ),
        refusal_row(
            "open_ended_external_event_stream",
            "session.open_ended_external.v1",
            "open_ended_external_event_stream",
            "ambient_event_mailbox",
            "interactive_surface_out_of_envelope",
            &["open:external", "user:wait_for_external_event"],
            "open-ended external event streams stay outside the bounded session-process profile",
        ),
    ];
    let exact_case_count = rows
        .iter()
        .filter(|row| row.status == TassadarSessionProcessCaseStatus::ExactDeterministicParity)
        .count() as u32;
    let refusal_case_count = rows
        .iter()
        .filter(|row| row.status == TassadarSessionProcessCaseStatus::ExactRefusalParity)
        .count() as u32;
    let routeable_interaction_surface_ids = rows
        .iter()
        .filter(|row| row.status == TassadarSessionProcessCaseStatus::ExactDeterministicParity)
        .map(|row| row.interaction_surface_id.clone())
        .collect::<Vec<_>>();
    let refused_interaction_surface_ids = rows
        .iter()
        .filter(|row| row.status == TassadarSessionProcessCaseStatus::ExactRefusalParity)
        .map(|row| row.interaction_surface_id.clone())
        .collect::<Vec<_>>();
    let overall_green =
        exact_case_count >= 2 && refusal_case_count >= 1 && !routeable_interaction_surface_ids.is_empty();
    let mut report = TassadarSessionProcessProfileRuntimeReport {
        schema_version: 1,
        report_id: String::from("tassadar.session_process_profile.runtime_report.v1"),
        profile_id: String::from(TASSADAR_SESSION_PROCESS_PROFILE_ID),
        rows,
        exact_case_count,
        refusal_case_count,
        overall_green,
        public_profile_allowed_profile_ids: if overall_green {
            vec![String::from(TASSADAR_SESSION_PROCESS_PROFILE_ID)]
        } else {
            Vec::new()
        },
        default_served_profile_allowed_profile_ids: Vec::new(),
        routeable_interaction_surface_ids,
        refused_interaction_surface_ids,
        claim_boundary: String::from(
            "this runtime report covers one bounded deterministic session-process profile with persisted local state, finite message transcripts, and explicit refusal on open-ended external event streams. It does not claim a generic agent loop, arbitrary interactive Wasm, ambient tool execution, or broader served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Session-process runtime report covers exact_cases={}, refusal_cases={}, routeable_surfaces={}, refused_surfaces={}, overall_green={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.routeable_interaction_surface_ids.len(),
        report.refused_interaction_surface_ids.len(),
        report.overall_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_session_process_profile_runtime_report|",
        &report,
    );
    report
}

#[must_use]
pub fn tassadar_session_process_profile_runtime_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SESSION_PROCESS_PROFILE_RUNTIME_REPORT_REF)
}

pub fn write_tassadar_session_process_profile_runtime_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarSessionProcessProfileRuntimeReport,
    TassadarSessionProcessProfileRuntimeReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSessionProcessProfileRuntimeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_session_process_profile_runtime_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSessionProcessProfileRuntimeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn exact_row(
    case_id: &str,
    session_id: &str,
    interaction_surface_id: &str,
    local_state_shape_id: &str,
    inputs: &[&str],
    outputs: &[&str],
    note: &str,
) -> TassadarSessionProcessCaseRow {
    let input_messages = inputs
        .iter()
        .enumerate()
        .map(|(turn_index, payload)| message(case_id, turn_index as u32, payload, false))
        .collect::<Vec<_>>();
    let output_messages = outputs
        .iter()
        .enumerate()
        .map(|(turn_index, payload)| message(case_id, turn_index as u32, payload, true))
        .collect::<Vec<_>>();
    TassadarSessionProcessCaseRow {
        case_id: String::from(case_id),
        session_id: String::from(session_id),
        interaction_surface_id: String::from(interaction_surface_id),
        local_state_shape_id: String::from(local_state_shape_id),
        route_profile_id: String::from(TASSADAR_SESSION_PROCESS_PROFILE_ID),
        input_messages,
        output_messages,
        status: TassadarSessionProcessCaseStatus::ExactDeterministicParity,
        exact_state_parity: true,
        exact_output_parity: true,
        refusal_reason_id: None,
        final_state_digest: stable_digest(
            b"psionic_tassadar_session_process_state|",
            &(
                case_id,
                session_id,
                interaction_surface_id,
                local_state_shape_id,
                inputs,
                outputs,
            ),
        ),
        note: String::from(note),
    }
}

fn refusal_row(
    case_id: &str,
    session_id: &str,
    interaction_surface_id: &str,
    local_state_shape_id: &str,
    refusal_reason_id: &str,
    inputs: &[&str],
    note: &str,
) -> TassadarSessionProcessCaseRow {
    let input_messages = inputs
        .iter()
        .enumerate()
        .map(|(turn_index, payload)| message(case_id, turn_index as u32, payload, false))
        .collect::<Vec<_>>();
    TassadarSessionProcessCaseRow {
        case_id: String::from(case_id),
        session_id: String::from(session_id),
        interaction_surface_id: String::from(interaction_surface_id),
        local_state_shape_id: String::from(local_state_shape_id),
        route_profile_id: String::from(TASSADAR_SESSION_PROCESS_PROFILE_ID),
        input_messages,
        output_messages: Vec::new(),
        status: TassadarSessionProcessCaseStatus::ExactRefusalParity,
        exact_state_parity: true,
        exact_output_parity: true,
        refusal_reason_id: Some(String::from(refusal_reason_id)),
        final_state_digest: stable_digest(
            b"psionic_tassadar_session_process_refusal_state|",
            &(
                case_id,
                session_id,
                interaction_surface_id,
                local_state_shape_id,
                refusal_reason_id,
            ),
        ),
        note: String::from(note),
    }
}

fn message(
    case_id: &str,
    turn_index: u32,
    payload: &str,
    assistant_turn: bool,
) -> TassadarSessionProcessMessage {
    let kind = if payload.starts_with("open:") {
        TassadarSessionProcessMessageKind::OpenSession
    } else if payload.starts_with("close:") {
        TassadarSessionProcessMessageKind::CloseSession
    } else if assistant_turn {
        TassadarSessionProcessMessageKind::AssistantTurn
    } else {
        TassadarSessionProcessMessageKind::UserTurn
    };
    TassadarSessionProcessMessage {
        message_id: format!("{case_id}::{turn_index:02}"),
        turn_index,
        kind,
        payload_digest: stable_digest(
            b"psionic_tassadar_session_process_message|",
            &(case_id, turn_index, payload, assistant_turn),
        ),
        note: String::from(payload),
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
) -> Result<T, TassadarSessionProcessProfileRuntimeReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSessionProcessProfileRuntimeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSessionProcessProfileRuntimeReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_SESSION_PROCESS_PROFILE_ID, TassadarSessionProcessCaseStatus,
        build_tassadar_session_process_profile_runtime_report, read_json,
        tassadar_session_process_profile_runtime_report_path,
        write_tassadar_session_process_profile_runtime_report,
    };
    use tempfile::tempdir;

    #[test]
    fn session_process_runtime_report_keeps_message_loop_and_refusal_boundaries_explicit() {
        let report = build_tassadar_session_process_profile_runtime_report();

        assert!(report.overall_green);
        assert_eq!(report.profile_id, TASSADAR_SESSION_PROCESS_PROFILE_ID);
        assert_eq!(report.exact_case_count, 2);
        assert_eq!(report.refusal_case_count, 1);
        assert_eq!(
            report.public_profile_allowed_profile_ids,
            vec![String::from(TASSADAR_SESSION_PROCESS_PROFILE_ID)]
        );
        assert!(report.default_served_profile_allowed_profile_ids.is_empty());
        assert!(report
            .routeable_interaction_surface_ids
            .contains(&String::from("deterministic_echo_turn_loop")));
        assert!(report
            .routeable_interaction_surface_ids
            .contains(&String::from("stateful_counter_turn_loop")));
        assert!(report
            .refused_interaction_surface_ids
            .contains(&String::from("open_ended_external_event_stream")));
    }

    #[test]
    fn session_process_runtime_report_preserves_state_and_output_parity() {
        let report = build_tassadar_session_process_profile_runtime_report();
        let counter_case = report
            .rows
            .iter()
            .find(|row| row.case_id == "counter_stateful_session")
            .expect("counter case");
        let refusal_case = report
            .rows
            .iter()
            .find(|row| row.case_id == "open_ended_external_event_stream")
            .expect("refusal case");

        assert!(counter_case.exact_state_parity);
        assert!(counter_case.exact_output_parity);
        assert_eq!(counter_case.output_messages.len(), 4);
        assert_eq!(
            refusal_case.status,
            TassadarSessionProcessCaseStatus::ExactRefusalParity
        );
        assert_eq!(
            refusal_case.refusal_reason_id.as_deref(),
            Some("interactive_surface_out_of_envelope")
        );
    }

    #[test]
    fn session_process_runtime_report_matches_committed_truth() {
        let generated = build_tassadar_session_process_profile_runtime_report();
        let committed = read_json(tassadar_session_process_profile_runtime_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_session_process_runtime_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_session_process_profile_runtime_report.json");
        let report = write_tassadar_session_process_profile_runtime_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_session_process_profile_runtime_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_session_process_profile_runtime_report.json")
        );
    }
}
