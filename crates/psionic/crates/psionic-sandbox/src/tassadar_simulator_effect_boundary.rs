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
    TASSADAR_SIMULATOR_EFFECT_RUN_ROOT_REF, TassadarSimulatorCaseStatus,
    build_tassadar_simulator_effect_runtime_bundle,
};

pub const TASSADAR_SIMULATOR_EFFECT_SANDBOX_BOUNDARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_simulator_effect_sandbox_boundary_report.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarSimulatorBoundaryStatus {
    AllowedDeterministic,
    RefusedOutOfEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorBoundaryRow {
    pub case_id: String,
    pub effect_ref: String,
    pub simulator_profile_id: String,
    pub status: TassadarSimulatorBoundaryStatus,
    pub simulator_required: bool,
    pub portability_envelope_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_reason_id: Option<String>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarSimulatorEffectSandboxBoundaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub profile_id: String,
    pub runtime_bundle_ref: String,
    pub rows: Vec<TassadarSimulatorBoundaryRow>,
    pub allowed_simulator_profile_ids: Vec<String>,
    pub refused_effect_ids: Vec<String>,
    pub allowed_case_count: u32,
    pub refused_case_count: u32,
    pub portability_envelope_ids: Vec<String>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarSimulatorEffectSandboxBoundaryError {
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
pub fn build_tassadar_simulator_effect_sandbox_boundary_report(
) -> TassadarSimulatorEffectSandboxBoundaryReport {
    let runtime_bundle = build_tassadar_simulator_effect_runtime_bundle();
    let rows = runtime_bundle
        .case_receipts
        .iter()
        .map(|case| TassadarSimulatorBoundaryRow {
            case_id: case.case_id.clone(),
            effect_ref: case.seed_profile_id.clone(),
            simulator_profile_id: case.seed_profile_id.clone(),
            status: if case.status == TassadarSimulatorCaseStatus::ExactReplayParity {
                TassadarSimulatorBoundaryStatus::AllowedDeterministic
            } else {
                TassadarSimulatorBoundaryStatus::RefusedOutOfEnvelope
            },
            simulator_required: true,
            portability_envelope_id: case.portability_envelope_id.clone(),
            refusal_reason_id: case
                .refusal_kinds
                .first()
                .map(|kind| format!("{kind:?}").to_lowercase()),
            note: case.note.clone(),
        })
        .collect::<Vec<_>>();
    let allowed_simulator_profile_ids = rows
        .iter()
        .filter(|row| row.status == TassadarSimulatorBoundaryStatus::AllowedDeterministic)
        .map(|row| row.simulator_profile_id.clone())
        .collect::<Vec<_>>();
    let refused_effect_ids = rows
        .iter()
        .filter(|row| row.status == TassadarSimulatorBoundaryStatus::RefusedOutOfEnvelope)
        .map(|row| row.effect_ref.clone())
        .collect::<Vec<_>>();
    let allowed_case_count = allowed_simulator_profile_ids.len() as u32;
    let refused_case_count = refused_effect_ids.len() as u32;
    let portability_envelope_ids = vec![String::from("cpu_reference_current_host")];
    let mut report = TassadarSimulatorEffectSandboxBoundaryReport {
        schema_version: 1,
        report_id: String::from("tassadar.simulator_effect.sandbox_boundary.report.v1"),
        profile_id: String::from(TASSADAR_SIMULATOR_EFFECT_PROFILE_ID),
        runtime_bundle_ref: format!(
            "{}/{}",
            TASSADAR_SIMULATOR_EFFECT_RUN_ROOT_REF, TASSADAR_SIMULATOR_EFFECT_BUNDLE_FILE
        ),
        rows,
        allowed_simulator_profile_ids,
        refused_effect_ids,
        allowed_case_count,
        refused_case_count,
        portability_envelope_ids,
        claim_boundary: String::from(
            "this sandbox boundary admits only named simulator-backed clock, randomness, and loopback-network profiles. It keeps ambient system clock, OS entropy, and socket I/O on explicit refusal paths instead of implying general host interaction",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Simulator-effect sandbox boundary freezes allowed_cases={}, refused_cases={}, portability_envelopes={}.",
        report.allowed_case_count,
        report.refused_case_count,
        report.portability_envelope_ids.len(),
    );
    report.report_digest =
        stable_digest(b"psionic_tassadar_simulator_effect_sandbox_boundary_report|", &report);
    report
}

#[must_use]
pub fn tassadar_simulator_effect_sandbox_boundary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_SIMULATOR_EFFECT_SANDBOX_BOUNDARY_REPORT_REF)
}

pub fn write_tassadar_simulator_effect_sandbox_boundary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarSimulatorEffectSandboxBoundaryReport,
    TassadarSimulatorEffectSandboxBoundaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarSimulatorEffectSandboxBoundaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_simulator_effect_sandbox_boundary_report();
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarSimulatorEffectSandboxBoundaryError::Write {
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
) -> Result<T, TassadarSimulatorEffectSandboxBoundaryError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarSimulatorEffectSandboxBoundaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarSimulatorEffectSandboxBoundaryError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarSimulatorBoundaryStatus, build_tassadar_simulator_effect_sandbox_boundary_report,
        read_json, tassadar_simulator_effect_sandbox_boundary_report_path,
        write_tassadar_simulator_effect_sandbox_boundary_report,
    };
    use tempfile::tempdir;

    #[test]
    fn simulator_effect_sandbox_boundary_keeps_allowed_and_refused_profiles_explicit() {
        let report = build_tassadar_simulator_effect_sandbox_boundary_report();

        assert_eq!(report.allowed_case_count, 3);
        assert_eq!(report.refused_case_count, 3);
        assert_eq!(report.allowed_simulator_profile_ids.len(), 3);
        assert_eq!(report.refused_effect_ids.len(), 3);
    }

    #[test]
    fn simulator_effect_sandbox_boundary_rows_are_typed() {
        let report = build_tassadar_simulator_effect_sandbox_boundary_report();
        let allowed = report
            .rows
            .iter()
            .find(|row| row.case_id == "seeded_clock_tick_case")
            .expect("allowed row");
        assert_eq!(allowed.status, TassadarSimulatorBoundaryStatus::AllowedDeterministic);
        let refused = report
            .rows
            .iter()
            .find(|row| row.case_id == "ambient_system_clock_refusal")
            .expect("refused row");
        assert_eq!(refused.status, TassadarSimulatorBoundaryStatus::RefusedOutOfEnvelope);
    }

    #[test]
    fn simulator_effect_sandbox_boundary_matches_committed_truth() {
        let generated = build_tassadar_simulator_effect_sandbox_boundary_report();
        let committed = read_json(tassadar_simulator_effect_sandbox_boundary_report_path())
            .expect("committed report");

        assert_eq!(generated, committed);
    }

    #[test]
    fn write_simulator_effect_sandbox_boundary_persists_current_truth() {
        let tempdir = tempdir().expect("tempdir");
        let output_path = tempdir
            .path()
            .join("tassadar_simulator_effect_sandbox_boundary_report.json");
        let report = write_tassadar_simulator_effect_sandbox_boundary_report(&output_path)
            .expect("write report");
        let persisted = read_json(&output_path).expect("persisted report");

        assert_eq!(report, persisted);
        assert_eq!(
            tassadar_simulator_effect_sandbox_boundary_report_path()
                .file_name()
                .and_then(std::ffi::OsStr::to_str),
            Some("tassadar_simulator_effect_sandbox_boundary_report.json")
        );
    }
}
