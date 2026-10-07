use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    TassadarInternalComputeProfileLadderReport,
    build_tassadar_internal_compute_profile_ladder_report,
};
use psionic_models::TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF;
#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_SUMMARY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_internal_compute_profile_ladder_summary.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputeProfileLadderSummaryReport {
    pub schema_version: u16,
    pub report_id: String,
    pub ladder_report_ref: String,
    pub ladder_report: TassadarInternalComputeProfileLadderReport,
    pub green: bool,
    pub implemented_profile_count: u32,
    pub planned_profile_count: u32,
    pub current_profile_id: String,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarInternalComputeProfileLadderSummaryReport {
    fn new(ladder_report: TassadarInternalComputeProfileLadderReport) -> Self {
        let implemented_profile_count = ladder_report
            .ladder_publication
            .profiles
            .iter()
            .filter(|profile| {
                profile.status == psionic_models::TassadarInternalComputeProfileStatus::Implemented
            })
            .count() as u32;
        let planned_profile_count =
            ladder_report.ladder_publication.profiles.len() as u32 - implemented_profile_count;
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.internal_compute_profile_ladder.summary.v1"),
            ladder_report_ref: String::from(TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF),
            green: ladder_report.current_served_claim_check.green,
            implemented_profile_count,
            planned_profile_count,
            current_profile_id: ladder_report
                .current_served_claim_check
                .claim
                .profile_id
                .clone(),
            ladder_report,
            claim_boundary: String::from(
                "this summary mirrors the internal-compute profile ladder report only. It exists to keep the named-profile posture operator-readable without widening the claim boundary",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "internal-compute profile ladder summary now records green={}, implemented_profiles={}, planned_profiles={}, current_profile_id=`{}`.",
            report.green,
            report.implemented_profile_count,
            report.planned_profile_count,
            report.current_profile_id,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_internal_compute_profile_ladder_summary|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarInternalComputeProfileLadderSummaryError {
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

pub fn build_tassadar_internal_compute_profile_ladder_summary_report() -> Result<
    TassadarInternalComputeProfileLadderSummaryReport,
    TassadarInternalComputeProfileLadderSummaryError,
> {
    Ok(TassadarInternalComputeProfileLadderSummaryReport::new(
        build_tassadar_internal_compute_profile_ladder_report(),
    ))
}

pub fn tassadar_internal_compute_profile_ladder_summary_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_SUMMARY_REPORT_REF)
}

pub fn write_tassadar_internal_compute_profile_ladder_summary_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarInternalComputeProfileLadderSummaryReport,
    TassadarInternalComputeProfileLadderSummaryError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalComputeProfileLadderSummaryError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_compute_profile_ladder_summary_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarInternalComputeProfileLadderSummaryError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-research should live under <repo>/crates/psionic-research")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarInternalComputeProfileLadderSummaryError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarInternalComputeProfileLadderSummaryError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarInternalComputeProfileLadderSummaryError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_SUMMARY_REPORT_REF,
        TassadarInternalComputeProfileLadderSummaryReport,
        build_tassadar_internal_compute_profile_ladder_summary_report, read_repo_json,
        tassadar_internal_compute_profile_ladder_summary_report_path,
        write_tassadar_internal_compute_profile_ladder_summary_report,
    };

    #[test]
    fn internal_compute_profile_ladder_summary_is_green_when_report_is_green() {
        let report =
            build_tassadar_internal_compute_profile_ladder_summary_report().expect("summary");

        assert!(report.green);
        assert_eq!(
            report.current_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
    }

    #[test]
    fn internal_compute_profile_ladder_summary_matches_committed_truth() {
        let generated =
            build_tassadar_internal_compute_profile_ladder_summary_report().expect("summary");
        let committed: TassadarInternalComputeProfileLadderSummaryReport = read_repo_json(
            TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_SUMMARY_REPORT_REF,
            "internal_compute_profile_ladder_summary",
        )
        .expect("committed summary");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_internal_compute_profile_ladder_summary_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_internal_compute_profile_ladder_summary.json");
        let written = write_tassadar_internal_compute_profile_ladder_summary_report(&output_path)
            .expect("write summary");
        let persisted: TassadarInternalComputeProfileLadderSummaryReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_internal_compute_profile_ladder_summary_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_internal_compute_profile_ladder_summary.json")
        );
    }
}
