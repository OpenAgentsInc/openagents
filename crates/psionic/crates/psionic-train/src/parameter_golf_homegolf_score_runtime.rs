use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ParameterGolfHomegolfClusteredRunSurfaceReport, ParameterGolfHomegolfDenseBaselineSurfaceReport,
};

pub const PARAMETER_GOLF_HOMEGOLF_SCORE_RUNTIME_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_score_relevant_runtime.json";
pub const PARAMETER_GOLF_HOMEGOLF_SCORE_RUNTIME_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-score-relevant-runtime.sh";
pub const PARAMETER_GOLF_HOMEGOLF_SCORE_RUNTIME_AUDIT: &str =
    "docs/audits/2026-03-27-homegolf-score-relevant-runtime-audit.md";

const HOMEGOLF_TRACK_ID: &str = "parameter_golf.home_cluster_compatible_10min.v1";
const MIXED_BACKEND_DENSE_RUN_REF: &str =
    "fixtures/training/first_same_job_mixed_backend_dense_run_v1.json";
const CLUSTERED_SURFACE_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_clustered_run_surface.json";
const DENSE_BASELINE_SURFACE_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_dense_baseline_surface.json";

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfScoreRuntimeError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid HOMEGOLF score runtime report: {detail}")]
    InvalidReport { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfScoreRuntimeStatus {
    ScoreRelevant,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfRuntimePhaseBreakdown {
    pub mean_cuda_submesh_step_ms: f64,
    pub mean_mlx_rank_step_ms: f64,
    pub mean_cross_backend_bridge_ms: f64,
    pub mean_optimizer_step_ms: f64,
    pub dominant_step_bottleneck: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfRuntimeDeviceMetric {
    pub node_id: String,
    pub execution_backend_label: String,
    pub observed_local_execution_wallclock_ms: u64,
    pub estimated_steps_per_second: f64,
    pub estimated_train_tokens_per_second: f64,
    pub contribution_share: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfScoreRuntimeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub track_id: String,
    pub source_clustered_surface_ref: String,
    pub source_dense_baseline_surface_ref: String,
    pub source_mixed_backend_dense_run_ref: String,
    pub wallclock_cap_seconds: u64,
    pub state_residency_posture: String,
    pub observed_step_count: u64,
    pub observed_train_tokens_per_step: u64,
    pub challenge_reference_train_batch_tokens: u64,
    pub challenge_reference_train_sequence_length: u64,
    pub challenge_reference_grad_accum_steps: u64,
    pub observed_cluster_wallclock_ms: u64,
    pub mean_cluster_step_ms: f64,
    pub effective_cluster_steps_per_second: f64,
    pub effective_cluster_train_tokens_per_second: f64,
    pub projected_steps_within_cap: f64,
    pub projected_train_tokens_within_cap: f64,
    pub projected_dataset_passes_within_cap: f64,
    pub per_device_metrics: Vec<ParameterGolfHomegolfRuntimeDeviceMetric>,
    pub phase_breakdown: ParameterGolfHomegolfRuntimePhaseBreakdown,
    pub runtime_status: ParameterGolfHomegolfScoreRuntimeStatus,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Deserialize)]
struct MixedBackendDenseRunSource {
    step_metrics: Vec<MixedBackendDenseStepMetricSource>,
}

#[derive(Debug, Deserialize)]
struct MixedBackendDenseStepMetricSource {
    train_tokens: u64,
    cuda_submesh_step_ms: u64,
    mlx_rank_step_ms: u64,
    cross_backend_bridge_ms: u64,
    optimizer_step_ms: u64,
}

impl ParameterGolfHomegolfScoreRuntimeReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"psionic_parameter_golf_homegolf_score_runtime|", &clone)
    }

    pub fn validate(&self) -> Result<(), ParameterGolfHomegolfScoreRuntimeError> {
        if self.schema_version != 1 {
            return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
                detail: format!("schema_version must stay 1 but was {}", self.schema_version),
            });
        }
        if self.track_id != HOMEGOLF_TRACK_ID {
            return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
                detail: String::from("track_id drifted"),
            });
        }
        if self.wallclock_cap_seconds != 600 {
            return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
                detail: String::from("wallclock_cap_seconds must stay 600"),
            });
        }
        if self.observed_step_count == 0 || self.observed_train_tokens_per_step == 0 {
            return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
                detail: String::from("observed dense runtime counts must stay positive"),
            });
        }
        if self.projected_dataset_passes_within_cap < 1.0 {
            return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
                detail: String::from(
                    "projected_dataset_passes_within_cap must stay at least one full dataset pass",
                ),
            });
        }
        if self.per_device_metrics.len() != 2 {
            return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
                detail: String::from(
                    "per_device_metrics must retain exactly two admitted mixed-device entries",
                ),
            });
        }
        if self.runtime_status != ParameterGolfHomegolfScoreRuntimeStatus::ScoreRelevant {
            return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
                detail: String::from("runtime_status drifted"),
            });
        }
        if self.report_digest != self.stable_digest() {
            return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
                detail: String::from("report_digest drifted"),
            });
        }
        Ok(())
    }
}

pub fn build_parameter_golf_homegolf_score_runtime_report(
) -> Result<ParameterGolfHomegolfScoreRuntimeReport, ParameterGolfHomegolfScoreRuntimeError> {
    let clustered_surface: ParameterGolfHomegolfClusteredRunSurfaceReport =
        serde_json::from_slice(&fs::read(resolve_repo_path(CLUSTERED_SURFACE_REF)).map_err(
            |error| ParameterGolfHomegolfScoreRuntimeError::Read {
                path: String::from(CLUSTERED_SURFACE_REF),
                error,
            },
        )?)?;
    let dense_baseline: ParameterGolfHomegolfDenseBaselineSurfaceReport = serde_json::from_slice(
        &fs::read(resolve_repo_path(DENSE_BASELINE_SURFACE_REF)).map_err(|error| {
            ParameterGolfHomegolfScoreRuntimeError::Read {
                path: String::from(DENSE_BASELINE_SURFACE_REF),
                error,
            }
        })?,
    )?;
    let mixed_backend_run: MixedBackendDenseRunSource = serde_json::from_slice(
        &fs::read(resolve_repo_path(MIXED_BACKEND_DENSE_RUN_REF)).map_err(|error| {
            ParameterGolfHomegolfScoreRuntimeError::Read {
                path: String::from(MIXED_BACKEND_DENSE_RUN_REF),
                error,
            }
        })?,
    )?;

    let observed_step_count = mixed_backend_run.step_metrics.len() as u64;
    if observed_step_count == 0 {
        return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
            detail: String::from("mixed-backend dense run retained zero steps"),
        });
    }
    let observed_train_tokens_per_step = mixed_backend_run.step_metrics[0].train_tokens;
    if mixed_backend_run
        .step_metrics
        .iter()
        .any(|metric| metric.train_tokens != observed_train_tokens_per_step)
    {
        return Err(ParameterGolfHomegolfScoreRuntimeError::InvalidReport {
            detail: String::from(
                "mixed-backend dense run changed train_tokens across retained steps",
            ),
        });
    }

    let total_train_tokens = observed_train_tokens_per_step.saturating_mul(observed_step_count);
    let observed_cluster_wallclock_ms = clustered_surface.observed_cluster_wallclock_ms;
    let mean_cluster_step_ms = observed_cluster_wallclock_ms as f64 / observed_step_count as f64;
    let effective_cluster_steps_per_second =
        observed_step_count as f64 / (observed_cluster_wallclock_ms as f64 / 1_000.0);
    let effective_cluster_train_tokens_per_second =
        total_train_tokens as f64 / (observed_cluster_wallclock_ms as f64 / 1_000.0);
    let projected_steps_within_cap =
        effective_cluster_steps_per_second * clustered_surface.wallclock_cap_seconds as f64;
    let projected_train_tokens_within_cap =
        effective_cluster_train_tokens_per_second * clustered_surface.wallclock_cap_seconds as f64;
    let projected_dataset_passes_within_cap =
        projected_train_tokens_within_cap / dense_baseline.train_token_count as f64;

    let phase_breakdown = ParameterGolfHomegolfRuntimePhaseBreakdown {
        mean_cuda_submesh_step_ms: mean(
            mixed_backend_run
                .step_metrics
                .iter()
                .map(|metric| metric.cuda_submesh_step_ms),
        ),
        mean_mlx_rank_step_ms: mean(
            mixed_backend_run
                .step_metrics
                .iter()
                .map(|metric| metric.mlx_rank_step_ms),
        ),
        mean_cross_backend_bridge_ms: mean(
            mixed_backend_run
                .step_metrics
                .iter()
                .map(|metric| metric.cross_backend_bridge_ms),
        ),
        mean_optimizer_step_ms: mean(
            mixed_backend_run
                .step_metrics
                .iter()
                .map(|metric| metric.optimizer_step_ms),
        ),
        dominant_step_bottleneck: String::from("mlx_rank"),
    };

    let per_device_metrics = clustered_surface
        .per_device_contributions
        .iter()
        .map(|contribution| ParameterGolfHomegolfRuntimeDeviceMetric {
            node_id: contribution.node_id.clone(),
            execution_backend_label: contribution.execution_backend_label.clone(),
            observed_local_execution_wallclock_ms: contribution.local_execution_wallclock_ms,
            estimated_steps_per_second: contribution.estimated_steps_per_second,
            estimated_train_tokens_per_second: contribution.estimated_samples_per_second,
            contribution_share: contribution.contribution_share,
        })
        .collect::<Vec<_>>();

    let mut report = ParameterGolfHomegolfScoreRuntimeReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_score_relevant_runtime.v1"),
        track_id: String::from(HOMEGOLF_TRACK_ID),
        source_clustered_surface_ref: String::from(CLUSTERED_SURFACE_REF),
        source_dense_baseline_surface_ref: String::from(DENSE_BASELINE_SURFACE_REF),
        source_mixed_backend_dense_run_ref: String::from(MIXED_BACKEND_DENSE_RUN_REF),
        wallclock_cap_seconds: clustered_surface.wallclock_cap_seconds,
        state_residency_posture: String::from("resident_dense_runtime_state"),
        observed_step_count,
        observed_train_tokens_per_step,
        challenge_reference_train_batch_tokens: dense_baseline.batch_geometry.train_batch_tokens
            as u64,
        challenge_reference_train_sequence_length: dense_baseline
            .batch_geometry
            .train_sequence_length as u64,
        challenge_reference_grad_accum_steps: dense_baseline.batch_geometry.grad_accum_steps
            as u64,
        observed_cluster_wallclock_ms,
        mean_cluster_step_ms,
        effective_cluster_steps_per_second,
        effective_cluster_train_tokens_per_second,
        projected_steps_within_cap,
        projected_train_tokens_within_cap,
        projected_dataset_passes_within_cap,
        per_device_metrics,
        phase_breakdown,
        runtime_status: ParameterGolfHomegolfScoreRuntimeStatus::ScoreRelevant,
        claim_boundary: String::from(
            "This retained HOMEGOLF runtime report upgrades the track from symbolic proof updates to a score-relevant dense runtime surface. It freezes real mixed-device dense timing, per-device contribution throughput, and projected 600-second training volume from the retained MLX-plus-CUDA same-job run. That retained dense runtime source is still the local MLX rank plus the optional-H100 CUDA submesh, not the currently reachable Apple-plus-home-RTX cluster. It does not yet claim admitted Apple-plus-home-RTX dense closure, one locally produced scored bundle from that admitted home cluster, or public-leaderboard-equivalent hardware.",
        ),
        summary: String::from(
            "The canonical retained HOMEGOLF runtime surface is now a resident dense mixed-device lane rather than a tiny bounded proof updater. The retained runtime clears one full training-dataset pass inside the 600-second cap, so score comparisons are now grounded in meaningful dense training volume instead of symbolic train-to-infer smoke tests.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

pub fn write_parameter_golf_homegolf_score_runtime_report(
    output_path: &Path,
) -> Result<ParameterGolfHomegolfScoreRuntimeReport, ParameterGolfHomegolfScoreRuntimeError> {
    let report = build_parameter_golf_homegolf_score_runtime_report()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfScoreRuntimeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfScoreRuntimeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn mean(values: impl Iterator<Item = u64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    let total = values.iter().sum::<u64>();
    total as f64 / values.len() as f64
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher
        .update(serde_json::to_vec(value).expect("HOMEGOLF score runtime report should serialize"));
    format!("{:x}", hasher.finalize())
}

fn resolve_repo_path(relpath: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(relpath)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        build_parameter_golf_homegolf_score_runtime_report,
        write_parameter_golf_homegolf_score_runtime_report,
        ParameterGolfHomegolfScoreRuntimeStatus, PARAMETER_GOLF_HOMEGOLF_SCORE_RUNTIME_REPORT_REF,
    };

    #[test]
    fn homegolf_score_runtime_stays_score_relevant() {
        let report = build_parameter_golf_homegolf_score_runtime_report().expect("build report");
        assert_eq!(
            report.runtime_status,
            ParameterGolfHomegolfScoreRuntimeStatus::ScoreRelevant
        );
        assert!(report.projected_dataset_passes_within_cap >= 1.0);
        assert!(report.effective_cluster_train_tokens_per_second > 0.0);
        assert_eq!(report.per_device_metrics.len(), 2);
    }

    #[test]
    fn write_homegolf_score_runtime_roundtrips() {
        let output = tempfile::tempdir().expect("tempdir");
        let path = output
            .path()
            .join("parameter_golf_homegolf_score_relevant_runtime.json");
        let written = write_parameter_golf_homegolf_score_runtime_report(path.as_path())
            .expect("write report");
        let encoded = std::fs::read(path.as_path()).expect("read report");
        let decoded: super::ParameterGolfHomegolfScoreRuntimeReport =
            serde_json::from_slice(&encoded).expect("decode report");
        assert_eq!(written, decoded);
    }

    #[test]
    fn committed_homegolf_score_runtime_fixture_roundtrips() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_SCORE_RUNTIME_REPORT_REF);
        let encoded = std::fs::read(fixture).expect("read fixture");
        let decoded: super::ParameterGolfHomegolfScoreRuntimeReport =
            serde_json::from_slice(&encoded).expect("decode fixture");
        decoded.validate().expect("validate fixture");
    }
}
