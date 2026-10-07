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
    TASSADAR_SIMULATOR_EFFECT_BUNDLE_FILE, TASSADAR_SIMULATOR_EFFECT_PROFILE_ID,
    TASSADAR_SIMULATOR_EFFECT_RUN_ROOT_REF, TassadarSimulatorCaseReceipt,
    TassadarSimulatorCaseStatus, TassadarSimulatorEffectRuntimeBundle,
    TassadarSimulatorKind, TassadarSimulatorRefusalKind,
    build_tassadar_simulator_effect_runtime_bundle,
};
use psionic_sandbox::{
    TASSADAR_SIMULATOR_EFFECT_SANDBOX_BOUNDARY_REPORT_REF,
    TassadarSimulatorEffectSandboxBoundaryReport,
    build_tassadar_simulator_effect_sandbox_boundary_report,
};

pub const TASSADAR_SIMULATOR_EFFECT_PROFILE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_simulator_effect_profile_report.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorTraceArtifactRef {
    pub case_id: String,
    pub trace_rel_path: String,
    pub trace_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorEffectCaseReport {
    pub case_id: String,
    pub simulator_kind: TassadarSimulatorKind,
    pub simulator_profile_id: String,
    pub status: TassadarSimulatorCaseStatus,
    pub exact_replay_parity: bool,
    pub trace_step_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_artifact_ref: Option<TassadarSimulatorTraceArtifactRef>,
    pub refusal_kinds: Vec<TassadarSimulatorRefusalKind>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorEffectProfileReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub runtime_bundle: TassadarSimulatorEffectRuntimeBundle,
    pub sandbox_boundary_report_ref: String,
    pub sandbox_boundary_report: TassadarSimulatorEffectSandboxBoundaryReport,
    pub case_reports: Vec<TassadarSimulatorEffectCaseReport>,
    pub allowed_simulator_profile_ids: Vec<String>,
    pub refused_effect_ids: Vec<String>,
    pub portability_envelope_ids: Vec<String>,
    pub exact_case_count: u32,
    pub refusal_case_count: u32,
    pub served_publication_allowed: bool,
    pub generated_from_refs: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSimulatorEffectProfileReportError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
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
}

#[derive(Clone)]
struct WritePlan {
    relative_path: String,
    bytes: Vec<u8>,
}

pub fn build_tassadar_simulator_effect_profile_report(
) -> Result<TassadarSimulatorEffectProfileReport, TassadarSimulatorEffectProfileReportError> {
    Ok(build_tassadar_simulator_effect_materialization()?.0)
}

#[must_use]
pub fn tassadar_simulator_effect_profile_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SIMULATOR_EFFECT_PROFILE_REPORT_REF)
}

pub fn write_tassadar_simulator_effect_profile_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarSimulatorEffectProfileReport, TassadarSimulatorEffectProfileReportError> {
    let output_path = output_path.as_ref();
    let (report, write_plans) = build_tassadar_simulator_effect_materialization()?;
    for plan in write_plans {
        let path = repo_root().join(&plan.relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                TassadarSimulatorEffectProfileReportError::CreateDir {
                    path: parent.display().to_string(),
                    error,
                }
            })?;
        }
        fs::write(&path, &plan.bytes).map_err(|error| {
            TassadarSimulatorEffectProfileReportError::Write {
                path: path.display().to_string(),
                error,
            }
        })?;
    }
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSimulatorEffectProfileReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSimulatorEffectProfileReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_tassadar_simulator_effect_materialization(
) -> Result<
    (TassadarSimulatorEffectProfileReport, Vec<WritePlan>),
    TassadarSimulatorEffectProfileReportError,
> {
    let runtime_bundle = build_tassadar_simulator_effect_runtime_bundle();
    let sandbox_boundary_report = build_tassadar_simulator_effect_sandbox_boundary_report();
    let runtime_bundle_ref = format!(
        "{}/{}",
        TASSADAR_SIMULATOR_EFFECT_RUN_ROOT_REF, TASSADAR_SIMULATOR_EFFECT_BUNDLE_FILE
    );
    let mut write_plans = vec![WritePlan {
        relative_path: runtime_bundle_ref.clone(),
        bytes: json_bytes(&runtime_bundle)?,
    }];
    let mut generated_from_refs = vec![
        runtime_bundle_ref.clone(),
        String::from(TASSADAR_SIMULATOR_EFFECT_SANDBOX_BOUNDARY_REPORT_REF),
    ];
    let mut case_reports = Vec::new();
    for case in &runtime_bundle.case_receipts {
        let (case_report, case_write_plans, case_refs) = build_case_materialization(case)?;
        case_reports.push(case_report);
        write_plans.extend(case_write_plans);
        generated_from_refs.extend(case_refs);
    }
    generated_from_refs.sort();
    generated_from_refs.dedup();
    let exact_case_count = case_reports
        .iter()
        .filter(|case| case.status == TassadarSimulatorCaseStatus::ExactReplayParity)
        .count() as u32;
    let refusal_case_count = case_reports
        .iter()
        .map(|case| case.refusal_kinds.len() as u32)
        .sum();
    let mut report = TassadarSimulatorEffectProfileReport {
        schema_version: 1,
        report_id: String::from("tassadar.simulator_effect.profile_report.v1"),
        profile_id: String::from(TASSADAR_SIMULATOR_EFFECT_PROFILE_ID),
        runtime_bundle_ref,
        runtime_bundle,
        sandbox_boundary_report_ref: String::from(
            TASSADAR_SIMULATOR_EFFECT_SANDBOX_BOUNDARY_REPORT_REF,
        ),
        sandbox_boundary_report,
        case_reports,
        allowed_simulator_profile_ids: Vec::new(),
        refused_effect_ids: Vec::new(),
        portability_envelope_ids: Vec::new(),
        exact_case_count,
        refusal_case_count,
        served_publication_allowed: false,
        generated_from_refs,
        claim_boundary: String::from(
            "this eval report covers one bounded simulator-backed effect profile with seeded clock, randomness, and loopback-network semantics on the current-host cpu-reference portability envelope only. It keeps ambient system clock, OS entropy, and socket I/O on explicit refusal paths instead of implying ambient host interaction, arbitrary network effects, or broader served internal compute",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.allowed_simulator_profile_ids = report.runtime_bundle.simulator_profile_ids.clone();
    report.refused_effect_ids = report.runtime_bundle.refused_effect_ids.clone();
    report.portability_envelope_ids = report.runtime_bundle.portability_envelope_ids.clone();
    report.summary = format!(
        "Simulator-effect profile report covers exact_cases={}, refusal_rows={}, simulator_profiles={}, served_publication_allowed={}.",
        report.exact_case_count,
        report.refusal_case_count,
        report.allowed_simulator_profile_ids.len(),
        report.served_publication_allowed,
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_simulator_effect_profile_report|", &report);
    Ok((report, write_plans))
}

fn build_case_materialization(
    case: &TassadarSimulatorCaseReceipt,
) -> Result<
    (
        TassadarSimulatorEffectCaseReport,
        Vec<WritePlan>,
        Vec<String>,
    ),
    TassadarSimulatorEffectProfileReportError,
> {
    let mut write_plans = Vec::new();
    let mut generated_from_refs = Vec::new();
    let trace_artifact_ref = if case.status == TassadarSimulatorCaseStatus::ExactReplayParity {
        let trace_rel_path = format!(
            "{}/simulator_traces/{}.json",
            TASSADAR_SIMULATOR_EFFECT_RUN_ROOT_REF, case.case_id
        );
        let bytes = json_bytes(&case.simulator_trace)?;
        let trace_digest = digest_bytes(&bytes);
        write_plans.push(WritePlan {
            relative_path: trace_rel_path.clone(),
            bytes,
        });
        generated_from_refs.push(trace_rel_path.clone());
        Some(TassadarSimulatorTraceArtifactRef {
            case_id: case.case_id.clone(),
            trace_rel_path,
            trace_digest,
        })
    } else {
        None
    };
    Ok((
        TassadarSimulatorEffectCaseReport {
            case_id: case.case_id.clone(),
            simulator_kind: case.simulator_kind,
            simulator_profile_id: case.seed_profile_id.clone(),
            status: case.status,
            exact_replay_parity: case.exact_replay_parity,
            trace_step_count: case.simulator_trace.len() as u32,
            trace_artifact_ref,
            refusal_kinds: case.refusal_kinds.clone(),
            note: case.note.clone(),
        },
        write_plans,
        generated_from_refs,
    ))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn json_bytes<T: Serialize>(
    value: &T,
) -> Result<Vec<u8>, TassadarSimulatorEffectProfileReportError> {
    Ok(format!("{}\n", serde_json::to_string_pretty(value)?).into_bytes())
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
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
) -> Result<T, TassadarSimulatorEffectProfileReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSimulatorEffectProfileReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSimulatorEffectProfileReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_simulator_effect_profile_report, read_json,
        tassadar_simulator_effect_profile_report_path,
        write_tassadar_simulator_effect_profile_report,
    };
    use tempfile::tempdir;

    #[test]
    fn simulator_effect_profile_report_keeps_allowed_and_refused_effects_explicit() {
        let report = build_tassadar_simulator_effect_profile_report().expect("report");

        assert_eq!(report.exact_case_count, 3);
        assert_eq!(report.refusal_case_count, 3);
        assert_eq!(report.allowed_simulator_profile_ids.len(), 3);
        assert_eq!(report.refused_effect_ids.len(), 3);
        assert_eq!(
            report.portability_envelope_ids,
            vec![String::from("cpu_reference_current_host")]
        );
        assert!(!report.served_publication_allowed);
    }

    #[test]
    fn simulator_effect_profile_report_materializes_seeded_trace_artifacts() {
        let report = build_tassadar_simulator_effect_profile_report().expect("report");
        let exact_case = report
            .case_reports
            .iter()
            .find(|case| case.case_id == "seeded_network_loopback_case")
            .expect("exact case");
        let refused_case = report
            .case_reports
            .iter()
            .find(|case| case.case_id == "ambient_socket_network_refusal")
            .expect("refused case");

        assert_eq!(exact_case.trace_step_count, 3);
        assert!(exact_case.trace_artifact_ref.is_some());
        assert!(refused_case.trace_artifact_ref.is_none());
    }

    #[test]
    fn simulator_effect_profile_report_matches_committed_truth() {
        let generated = build_tassadar_simulator_effect_profile_report().expect("report");
        let committed = read_json(tassadar_simulator_effect_profile_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_simulator_effect_profile_report_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_simulator_effect_profile_report.json");
        let report =
            write_tassadar_simulator_effect_profile_report(&output_path).expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_simulator_effect_profile_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_simulator_effect_profile_report.json")
        );
    }
}
