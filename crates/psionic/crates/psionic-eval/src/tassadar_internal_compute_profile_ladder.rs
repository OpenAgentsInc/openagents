use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF,
    TassadarInternalComputeProfileClaimCheckResult,
    TassadarInternalComputeProfileLadderPublication, check_tassadar_internal_compute_profile_claim,
    tassadar_current_served_internal_compute_profile_claim,
    tassadar_internal_compute_profile_ladder_publication,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarInternalComputeProfileLadderReport {
    pub schema_version: u16,
    pub report_id: String,
    pub ladder_publication: TassadarInternalComputeProfileLadderPublication,
    pub current_served_claim_check: TassadarInternalComputeProfileClaimCheckResult,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl TassadarInternalComputeProfileLadderReport {
    fn new(
        ladder_publication: TassadarInternalComputeProfileLadderPublication,
        current_served_claim_check: TassadarInternalComputeProfileClaimCheckResult,
    ) -> Self {
        let implemented_profile_count = ladder_publication
            .profiles
            .iter()
            .filter(|profile| {
                profile.status == psionic_models::TassadarInternalComputeProfileStatus::Implemented
            })
            .count();
        let planned_profile_count = ladder_publication.profiles.len() - implemented_profile_count;
        let mut report = Self {
            schema_version: 1,
            report_id: String::from("tassadar.internal_compute_profile_ladder.report.v1"),
            ladder_publication,
            current_served_claim_check,
            claim_boundary: String::from(
                "this report freezes the named post-article internal-compute profile ladder plus the current served claim-check result. It exists to stop vague Rust/Wasm language from outrunning the actual evidence-backed claim",
            ),
            summary: String::new(),
            report_digest: String::new(),
        };
        report.summary = format!(
            "internal-compute profile ladder now records implemented_profiles={}, planned_profiles={}, current_claim_green={}, and current_profile_id=`{}`.",
            implemented_profile_count,
            planned_profile_count,
            report.current_served_claim_check.green,
            report.current_served_claim_check.claim.profile_id,
        );
        report.report_digest = stable_digest(
            b"psionic_tassadar_internal_compute_profile_ladder_report|",
            &report,
        );
        report
    }
}

#[derive(Debug, Error)]
pub enum TassadarInternalComputeProfileLadderReportError {
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write report `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read committed report `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode committed report `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
}

pub fn build_tassadar_internal_compute_profile_ladder_report()
-> TassadarInternalComputeProfileLadderReport {
    let ladder_publication = tassadar_internal_compute_profile_ladder_publication();
    let current_served_claim_check = check_tassadar_internal_compute_profile_claim(
        &ladder_publication,
        tassadar_current_served_internal_compute_profile_claim(),
    );
    TassadarInternalComputeProfileLadderReport::new(ladder_publication, current_served_claim_check)
}

pub fn tassadar_internal_compute_profile_ladder_report_path() -> PathBuf {
    repo_root().join(TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF)
}

pub fn write_tassadar_internal_compute_profile_ladder_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarInternalComputeProfileLadderReport,
    TassadarInternalComputeProfileLadderReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarInternalComputeProfileLadderReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_internal_compute_profile_ladder_report();
    let bytes = serde_json::to_vec_pretty(&report)
        .expect("internal compute profile ladder report should serialize");
    fs::write(output_path, bytes).map_err(|error| {
        TassadarInternalComputeProfileLadderReportError::Write {
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
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
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
        TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF,
        TassadarInternalComputeProfileLadderReport,
        build_tassadar_internal_compute_profile_ladder_report,
        tassadar_internal_compute_profile_ladder_report_path,
        write_tassadar_internal_compute_profile_ladder_report,
    };

    #[test]
    fn internal_compute_profile_ladder_report_is_machine_legible() {
        let report = build_tassadar_internal_compute_profile_ladder_report();

        assert_eq!(
            report.ladder_publication.report_ref,
            TASSADAR_INTERNAL_COMPUTE_PROFILE_LADDER_REPORT_REF
        );
        assert!(report.current_served_claim_check.green);
    }

    #[test]
    fn internal_compute_profile_ladder_report_matches_committed_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_internal_compute_profile_ladder_report();
        let bytes = std::fs::read(tassadar_internal_compute_profile_ladder_report_path())?;
        let committed: TassadarInternalComputeProfileLadderReport = serde_json::from_slice(&bytes)?;
        assert_eq!(report, committed);
        Ok(())
    }

    #[test]
    fn write_internal_compute_profile_ladder_report_persists_current_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let report_path = temp_dir
            .path()
            .join("tassadar_internal_compute_profile_ladder_report.json");
        let report = write_tassadar_internal_compute_profile_ladder_report(&report_path)?;
        let persisted: TassadarInternalComputeProfileLadderReport =
            serde_json::from_slice(&std::fs::read(&report_path)?)?;
        assert_eq!(report, persisted);
        Ok(())
    }
}
