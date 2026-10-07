use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::ParameterGolfHomegolfComparisonClass;

pub const PARAMETER_GOLF_HOMEGOLF_PUBLIC_COMPARISON_FIXTURE_PATH: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_public_comparison.json";
pub const PARAMETER_GOLF_HOMEGOLF_PUBLIC_COMPARISON_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-public-comparison.sh";
pub const PARAMETER_GOLF_HOMEGOLF_PUBLIC_COMPARISON_AUDIT: &str =
    "docs/audits/2026-03-27-homegolf-public-comparison-audit.md";

const HOMEGOLF_CLUSTERED_RUN_SURFACE_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_clustered_run_surface.json";

const PUBLIC_BASELINE_SOURCE_REF: &str =
    "competition/repos/parameter-golf/records/track_10min_16mb/2026-03-17_NaiveBaseline/submission.json";
const PUBLIC_LEADERBOARD_SOURCE_REF: &str =
    "competition/repos/parameter-golf/records/track_10min_16mb/2026-03-23_LeakyReLU_LegalTTT_ParallelMuon/{submission.json,README.md}";

const PUBLIC_BASELINE_VAL_BPB: f64 = 1.224_365_7;
const PUBLIC_BASELINE_ARTIFACT_BYTES: u64 = 15_863_489;
const PUBLIC_LEADERBOARD_VAL_BPB: f64 = 1.119_4;
const PUBLIC_LEADERBOARD_ARTIFACT_BYTES: u64 = 15_990_006;
const PUBLIC_WALLCLOCK_CAP_SECONDS: u64 = 600;

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfPublicComparisonError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid HOMEGOLF public comparison report: {detail}")]
    InvalidReport { detail: String },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfPublicReference {
    pub label: String,
    pub val_bpb: f64,
    pub artifact_bytes: u64,
    pub wallclock_cap_seconds: u64,
    pub hardware_posture: String,
    pub source_ref: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfPublicComparisonDelta {
    pub delta_val_bpb: f64,
    pub delta_artifact_bytes: i64,
    pub delta_wallclock_cap_seconds: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfPublicComparisonReport {
    pub schema_version: u16,
    pub report_id: String,
    pub source_clustered_run_surface_ref: String,
    pub track_id: String,
    pub comparison_classes: Vec<ParameterGolfHomegolfComparisonClass>,
    pub homegolf_final_validation_bits_per_byte: f64,
    pub homegolf_scored_artifact_bytes: u64,
    pub homegolf_wallclock_cap_seconds: u64,
    pub homegolf_observed_cluster_wallclock_ms: u64,
    pub public_naive_baseline: ParameterGolfHomegolfPublicReference,
    pub current_public_leaderboard_best: ParameterGolfHomegolfPublicReference,
    pub delta_vs_public_naive_baseline: ParameterGolfHomegolfPublicComparisonDelta,
    pub delta_vs_current_public_leaderboard_best: ParameterGolfHomegolfPublicComparisonDelta,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Deserialize)]
struct HomegolfClusteredRunSurfaceSource {
    track_id: String,
    wallclock_cap_seconds: u64,
    observed_cluster_wallclock_ms: u64,
    final_validation_bits_per_byte: f64,
    model_artifact_bytes: u64,
}

impl ParameterGolfHomegolfPublicComparisonReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_homegolf_public_comparison|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), ParameterGolfHomegolfPublicComparisonError> {
        if self.schema_version != 1 {
            return Err(ParameterGolfHomegolfPublicComparisonError::InvalidReport {
                detail: format!("schema_version must stay 1 but was {}", self.schema_version),
            });
        }
        if self.comparison_classes
            != vec![
                ParameterGolfHomegolfComparisonClass::PublicBaselineComparable,
                ParameterGolfHomegolfComparisonClass::NotPublicLeaderboardEquivalent,
            ]
        {
            return Err(ParameterGolfHomegolfPublicComparisonError::InvalidReport {
                detail: String::from("comparison_classes drifted"),
            });
        }
        if self.public_naive_baseline.val_bpb != PUBLIC_BASELINE_VAL_BPB
            || self.public_naive_baseline.artifact_bytes != PUBLIC_BASELINE_ARTIFACT_BYTES
            || self.current_public_leaderboard_best.val_bpb != PUBLIC_LEADERBOARD_VAL_BPB
            || self.current_public_leaderboard_best.artifact_bytes
                != PUBLIC_LEADERBOARD_ARTIFACT_BYTES
        {
            return Err(ParameterGolfHomegolfPublicComparisonError::InvalidReport {
                detail: String::from("public reference metrics drifted"),
            });
        }
        if self.homegolf_wallclock_cap_seconds != 600
            || self.homegolf_observed_cluster_wallclock_ms > 600_000
        {
            return Err(ParameterGolfHomegolfPublicComparisonError::InvalidReport {
                detail: String::from("HOMEGOLF wallclock posture drifted"),
            });
        }
        if self.homegolf_final_validation_bits_per_byte <= 0.0
            || self.homegolf_scored_artifact_bytes == 0
        {
            return Err(ParameterGolfHomegolfPublicComparisonError::InvalidReport {
                detail: String::from(
                    "HOMEGOLF comparison metrics must keep positive score and artifact bytes",
                ),
            });
        }
        if self.report_digest != self.stable_digest() {
            return Err(ParameterGolfHomegolfPublicComparisonError::InvalidReport {
                detail: String::from("report_digest drifted"),
            });
        }
        Ok(())
    }
}

pub fn build_parameter_golf_homegolf_public_comparison_report(
) -> Result<ParameterGolfHomegolfPublicComparisonReport, ParameterGolfHomegolfPublicComparisonError>
{
    let source: HomegolfClusteredRunSurfaceSource = serde_json::from_slice(
        &fs::read(resolve_repo_path(HOMEGOLF_CLUSTERED_RUN_SURFACE_REF)).map_err(|error| {
            ParameterGolfHomegolfPublicComparisonError::Read {
                path: String::from(HOMEGOLF_CLUSTERED_RUN_SURFACE_REF),
                error,
            }
        })?,
    )?;

    let public_naive_baseline = ParameterGolfHomegolfPublicReference {
        label: String::from("public_naive_baseline"),
        val_bpb: PUBLIC_BASELINE_VAL_BPB,
        artifact_bytes: PUBLIC_BASELINE_ARTIFACT_BYTES,
        wallclock_cap_seconds: PUBLIC_WALLCLOCK_CAP_SECONDS,
        hardware_posture: String::from("8xH100 SXM"),
        source_ref: String::from(PUBLIC_BASELINE_SOURCE_REF),
    };
    let current_public_leaderboard_best = ParameterGolfHomegolfPublicReference {
        label: String::from("current_public_leaderboard_best"),
        val_bpb: PUBLIC_LEADERBOARD_VAL_BPB,
        artifact_bytes: PUBLIC_LEADERBOARD_ARTIFACT_BYTES,
        wallclock_cap_seconds: PUBLIC_WALLCLOCK_CAP_SECONDS,
        hardware_posture: String::from("8xH100 SXM"),
        source_ref: String::from(PUBLIC_LEADERBOARD_SOURCE_REF),
    };

    let delta_vs_public_naive_baseline = ParameterGolfHomegolfPublicComparisonDelta {
        delta_val_bpb: source.final_validation_bits_per_byte - public_naive_baseline.val_bpb,
        delta_artifact_bytes: source.model_artifact_bytes as i64
            - public_naive_baseline.artifact_bytes as i64,
        delta_wallclock_cap_seconds: source.wallclock_cap_seconds as i64
            - public_naive_baseline.wallclock_cap_seconds as i64,
    };
    let delta_vs_current_public_leaderboard_best = ParameterGolfHomegolfPublicComparisonDelta {
        delta_val_bpb: source.final_validation_bits_per_byte
            - current_public_leaderboard_best.val_bpb,
        delta_artifact_bytes: source.model_artifact_bytes as i64
            - current_public_leaderboard_best.artifact_bytes as i64,
        delta_wallclock_cap_seconds: source.wallclock_cap_seconds as i64
            - current_public_leaderboard_best.wallclock_cap_seconds as i64,
    };

    let mut report = ParameterGolfHomegolfPublicComparisonReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_public_comparison.v1"),
        source_clustered_run_surface_ref: String::from(HOMEGOLF_CLUSTERED_RUN_SURFACE_REF),
        track_id: source.track_id,
        comparison_classes: vec![
            ParameterGolfHomegolfComparisonClass::PublicBaselineComparable,
            ParameterGolfHomegolfComparisonClass::NotPublicLeaderboardEquivalent,
        ],
        homegolf_final_validation_bits_per_byte: source.final_validation_bits_per_byte,
        homegolf_scored_artifact_bytes: source.model_artifact_bytes,
        homegolf_wallclock_cap_seconds: source.wallclock_cap_seconds,
        homegolf_observed_cluster_wallclock_ms: source.observed_cluster_wallclock_ms,
        public_naive_baseline,
        current_public_leaderboard_best,
        delta_vs_public_naive_baseline,
        delta_vs_current_public_leaderboard_best,
        claim_boundary: String::from(
            "This report makes HOMEGOLF publicly comparable, not public-leaderboard equivalent. The public reference values are frozen from the current Parameter Golf repo snapshot reviewed on 2026-03-27, while the HOMEGOLF side now uses the retained H100-backed live dense mixed-device surface and its exact dense challenge export bytes rather than the older open-adapter composed surrogate. The current lane is still far from leaderboard quality, and the mixed-device runtime still does not imply official 8xH100 hardware equivalence or current Apple-plus-home-RTX score closure.",
        ),
        summary: String::from(
            "HOMEGOLF now emits one deterministic comparison report against the public naive baseline and the current public best leaderboard row from the retained H100-backed live dense mixed-device surface. It keeps exact deltas in val_bpb, scored artifact bytes, and wallclock-cap posture while explicitly refusing leaderboard-equivalent language for the custom-hardware track.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

pub fn write_parameter_golf_homegolf_public_comparison_report(
    output_path: &Path,
) -> Result<ParameterGolfHomegolfPublicComparisonReport, ParameterGolfHomegolfPublicComparisonError>
{
    let report = build_parameter_golf_homegolf_public_comparison_report()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfPublicComparisonError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfPublicComparisonError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn resolve_repo_path(relpath: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relpath)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("HOMEGOLF public comparison should serialize"));
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_parameter_golf_homegolf_public_comparison_report,
        write_parameter_golf_homegolf_public_comparison_report,
        ParameterGolfHomegolfComparisonClass,
        PARAMETER_GOLF_HOMEGOLF_PUBLIC_COMPARISON_FIXTURE_PATH,
    };

    #[test]
    fn homegolf_public_comparison_keeps_public_snapshot_and_track_language() {
        let report =
            build_parameter_golf_homegolf_public_comparison_report().expect("build report");
        assert_eq!(report.homegolf_wallclock_cap_seconds, 600);
        assert_eq!(
            report.comparison_classes,
            vec![
                ParameterGolfHomegolfComparisonClass::PublicBaselineComparable,
                ParameterGolfHomegolfComparisonClass::NotPublicLeaderboardEquivalent,
            ]
        );
        assert!(report.delta_vs_public_naive_baseline.delta_val_bpb > 0.0);
        assert!(
            report
                .delta_vs_current_public_leaderboard_best
                .delta_val_bpb
                > 0.0
        );
    }

    #[test]
    fn write_homegolf_public_comparison_roundtrips() {
        let output = tempfile::tempdir().expect("tempdir");
        let path = output
            .path()
            .join("parameter_golf_homegolf_public_comparison.json");
        let written = write_parameter_golf_homegolf_public_comparison_report(path.as_path())
            .expect("write report");
        let encoded = std::fs::read(path.as_path()).expect("read report");
        let decoded: super::ParameterGolfHomegolfPublicComparisonReport =
            serde_json::from_slice(&encoded).expect("decode report");
        assert_eq!(written, decoded);
    }

    #[test]
    fn committed_homegolf_public_comparison_fixture_roundtrips() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_PUBLIC_COMPARISON_FIXTURE_PATH);
        let encoded = std::fs::read(fixture).expect("read fixture");
        let decoded: super::ParameterGolfHomegolfPublicComparisonReport =
            serde_json::from_slice(&encoded).expect("decode fixture");
        let rebuilt =
            build_parameter_golf_homegolf_public_comparison_report().expect("rebuild report");
        assert_eq!(decoded, rebuilt);
    }
}
