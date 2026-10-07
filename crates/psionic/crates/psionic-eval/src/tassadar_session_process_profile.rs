use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_runtime::{
    TASSADAR_SESSION_PROCESS_PROFILE_ID, TASSADAR_SESSION_PROCESS_PROFILE_RUNTIME_REPORT_REF,
    TassadarSessionProcessCaseStatus,
    TassadarSessionProcessProfileRuntimeReport as TassadarSessionProcessRuntimeReport,
    build_tassadar_session_process_profile_runtime_report,
};

pub const TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_session_process_profile_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSessionProcessProfileCaseAudit {
    pub case_id: String,
    pub session_id: String,
    pub interaction_surface_id: String,
    pub local_state_shape_id: String,
    pub runtime_status: TassadarSessionProcessCaseStatus,
    pub exact_state_parity: bool,
    pub exact_output_parity: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub route_eligible: bool,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSessionProcessProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_report_ref: String,
    pub runtime_report: TassadarSessionProcessRuntimeReport,
    pub case_audits: Vec<TassadarSessionProcessProfileCaseAudit>,
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
pub enum TassadarSessionProcessProfileReportError {
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

pub fn build_tassadar_session_process_profile_report(
) -> Result<TassadarSessionProcessProfileReport, TassadarSessionProcessProfileReportError> {
    let runtime_report = build_tassadar_session_process_profile_runtime_report();
    let case_audits = runtime_report
        .rows
        .iter()
        .map(|row| TassadarSessionProcessProfileCaseAudit {
            case_id: row.case_id.clone(),
            session_id: row.session_id.clone(),
            interaction_surface_id: row.interaction_surface_id.clone(),
            local_state_shape_id: row.local_state_shape_id.clone(),
            runtime_status: row.status,
            exact_state_parity: row.exact_state_parity,
            exact_output_parity: row.exact_output_parity,
            refusal_reason_id: row.refusal_reason_id.clone(),
            route_eligible: row.status == TassadarSessionProcessCaseStatus::ExactDeterministicParity,
            note: row.note.clone(),
        })
        .collect::<Vec<_>>();
    let exact_case_count = case_audits
        .iter()
        .filter(|case| {
            case.runtime_status == TassadarSessionProcessCaseStatus::ExactDeterministicParity
        })
        .count() as u32;
    let refusal_case_count = case_audits
        .iter()
        .filter(|case| case.runtime_status == TassadarSessionProcessCaseStatus::ExactRefusalParity)
        .count() as u32;
    let routeable_interaction_surface_ids = case_audits
        .iter()
        .filter(|case| case.route_eligible)
        .map(|case| case.interaction_surface_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let refused_interaction_surface_ids = case_audits
        .iter()
        .filter(|case| case.runtime_status == TassadarSessionProcessCaseStatus::ExactRefusalParity)
        .map(|case| case.interaction_surface_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let overall_green = runtime_report.overall_green
        && exact_case_count == runtime_report.exact_case_count
        && refusal_case_count == runtime_report.refusal_case_count
        && !routeable_interaction_surface_ids.is_empty();
    let mut report = TassadarSessionProcessProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.session_process_profile.report.v1"),
        profile_id: String::from(TASSADAR_SESSION_PROCESS_PROFILE_ID),
        runtime_report_ref: String::from(TASSADAR_SESSION_PROCESS_PROFILE_RUNTIME_REPORT_REF),
        runtime_report,
        case_audits,
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
            "this eval report covers one bounded interactive session-process profile with finite message transcripts and persisted local session state. It admits profile-specific named public posture only for the deterministic surfaces in this report and keeps open-ended external event loops on explicit refusal paths instead of implying a generic agent loop or broader served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Session-process profile report covers exact_cases={}, refusal_cases={}, routeable_surfaces={}, refused_surfaces={}, overall_green={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.routeable_interaction_surface_ids.len(),
        report.refused_interaction_surface_ids.len(),
        report.overall_green,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_session_process_profile_report|", &report);
    Ok(report)
}

#[must_use]
pub fn tassadar_session_process_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SESSION_PROCESS_PROFILE_REPORT_REF)
}

pub fn write_tassadar_session_process_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSessionProcessProfileReport, TassadarSessionProcessProfileReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSessionProcessProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_session_process_profile_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSessionProcessProfileReportError::Write {
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
) -> Result<T, TassadarSessionProcessProfileReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| TassadarSessionProcessProfileReportError::Read {
        path: path.display().to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSessionProcessProfileReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_session_process_profile_report, read_json,
        tassadar_session_process_profile_report_path,
        write_tassadar_session_process_profile_report,
    };
    use psionic_runtime::TASSADAR_SESSION_PROCESS_PROFILE_ID;
    use tempfile::tempdir;

    #[test]
    fn session_process_profile_report_keeps_named_public_routeability_bounded() {
        let report = build_tassadar_session_process_profile_report().expect("report");

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
    fn session_process_profile_report_matches_committed_truth() {
        let generated = build_tassadar_session_process_profile_report().expect("report");
        let committed = read_json(tassadar_session_process_profile_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_session_process_profile_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_session_process_profile_report.json");
        let report =
            write_tassadar_session_process_profile_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_session_process_profile_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_session_process_profile_report.json")
        );
    }
}
