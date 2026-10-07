use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_models::{
    PARAMETER_GOLF_BASELINE_MODEL_ID, PARAMETER_GOLF_BASELINE_REVISION,
    PARAMETER_GOLF_PROMOTED_CHALLENGE_PROFILE_ID, ParameterGolfConfig,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    ParameterGolfBatchGeometry, ParameterGolfSingleH100TrainingReport,
    ParameterGolfSingleH100TrainingStopReason,
};

/// Canonical committed HOMEGOLF dense-baseline surface report.
pub const PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SURFACE_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_dense_baseline_surface.json";
/// Canonical source report proving one exact dense baseline run completed in Psionic.
pub const PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SOURCE_REPORT_REF: &str = "fixtures/parameter_golf/reports/parameter_golf_runpod_single_h100_first_real_training_report.json";
/// Canonical checker script for the HOMEGOLF dense-baseline surface.
pub const PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SURFACE_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-dense-baseline-surface.sh";

/// Final retained validation metric for the first honest HOMEGOLF dense baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfDenseBaselineMetric {
    /// Metric source label.
    pub metric_source: String,
    /// Exact validation loss from the retained dense baseline report.
    pub val_loss: f64,
    /// Exact tokenizer-agnostic BPB from the retained dense baseline report.
    pub val_bpb: f64,
    /// Evaluated token count.
    pub evaluated_token_count: u64,
    /// Evaluated byte count.
    pub evaluated_byte_count: u64,
}

/// Machine-readable HOMEGOLF report proving the first exact dense baseline surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfDenseBaselineSurfaceReport {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable report identifier.
    pub report_id: String,
    /// Stable HOMEGOLF track identifier.
    pub track_id: String,
    /// Stable strict challenge profile bound to this surface.
    pub strict_profile_id: String,
    /// Canonical source training report ref.
    pub source_training_report_ref: String,
    /// Stable digest of the source training report.
    pub source_training_report_digest: String,
    /// Canonical dense trainer entrypoint.
    pub trainer_entrypoint: String,
    /// Canonical dense trainer implementation.
    pub trainer_implementation: String,
    /// Exact frozen baseline model id.
    pub baseline_model_id: String,
    /// Exact frozen baseline revision.
    pub baseline_revision: String,
    /// Exact frozen dense baseline config.
    pub baseline_config: ParameterGolfConfig,
    /// Exact batch geometry used by the retained dense baseline surface.
    pub batch_geometry: ParameterGolfBatchGeometry,
    /// Retained explicit step cap from the source report.
    pub max_steps: u64,
    /// Retained executed step count from the source report.
    pub executed_steps: u64,
    /// Retained stop reason from the source report.
    pub stop_reason: Option<ParameterGolfSingleH100TrainingStopReason>,
    /// Retained train token count from the source report.
    pub train_token_count: u64,
    /// Retained wallclock from the source report.
    pub observed_wallclock_ms: u64,
    /// Retained compressed artifact size.
    pub compressed_model_bytes: u64,
    /// Retained compressed artifact ref.
    pub compressed_model_artifact_ref: String,
    /// Retained compressed artifact digest.
    pub compressed_model_artifact_digest: String,
    /// Final retained validation metric.
    pub final_metric: ParameterGolfHomegolfDenseBaselineMetric,
    /// Honest claim boundary for operators.
    pub claim_boundary: String,
    /// Plain-language summary.
    pub summary: String,
    /// Stable digest over the HOMEGOLF dense-baseline surface report.
    pub report_digest: String,
}

/// Failure while building or writing the HOMEGOLF dense-baseline surface report.
#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfDenseBaselineSurfaceError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid HOMEGOLF dense-baseline source report: {message}")]
    InvalidSource { message: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Builds the canonical HOMEGOLF dense-baseline surface report from the retained
/// exact dense single-device source report.
pub fn build_parameter_golf_homegolf_dense_baseline_surface_report() -> Result<
    ParameterGolfHomegolfDenseBaselineSurfaceReport,
    ParameterGolfHomegolfDenseBaselineSurfaceError,
> {
    let source_path = parameter_golf_homegolf_dense_baseline_source_report_path();
    let source_bytes = fs::read(&source_path).map_err(|error| {
        ParameterGolfHomegolfDenseBaselineSurfaceError::Read {
            path: source_path.display().to_string(),
            error,
        }
    })?;
    let source_report: ParameterGolfSingleH100TrainingReport =
        serde_json::from_slice(&source_bytes)?;

    if !source_report.training_executed() {
        return Err(
            ParameterGolfHomegolfDenseBaselineSurfaceError::InvalidSource {
                message: String::from(
                    "source training report is not an executed dense baseline run",
                ),
            },
        );
    }
    if source_report.baseline_model_id != PARAMETER_GOLF_BASELINE_MODEL_ID {
        return Err(
            ParameterGolfHomegolfDenseBaselineSurfaceError::InvalidSource {
                message: format!(
                    "source baseline_model_id drifted: expected `{PARAMETER_GOLF_BASELINE_MODEL_ID}`, observed `{}`",
                    source_report.baseline_model_id
                ),
            },
        );
    }
    if source_report.baseline_model_revision != PARAMETER_GOLF_BASELINE_REVISION {
        return Err(
            ParameterGolfHomegolfDenseBaselineSurfaceError::InvalidSource {
                message: format!(
                    "source baseline_model_revision drifted: expected `{PARAMETER_GOLF_BASELINE_REVISION}`, observed `{}`",
                    source_report.baseline_model_revision
                ),
            },
        );
    }
    if source_report.geometry != ParameterGolfBatchGeometry::challenge_single_device_defaults() {
        return Err(
            ParameterGolfHomegolfDenseBaselineSurfaceError::InvalidSource {
                message: String::from(
                    "source dense baseline geometry drifted from exact challenge_single_device_defaults",
                ),
            },
        );
    }

    let final_metric = if let Some(roundtrip) = source_report.final_roundtrip_receipt.as_ref() {
        ParameterGolfHomegolfDenseBaselineMetric {
            metric_source: roundtrip.metric_source.clone(),
            val_loss: roundtrip.validation.mean_loss,
            val_bpb: roundtrip.validation.bits_per_byte,
            evaluated_token_count: roundtrip.validation.evaluated_token_count,
            evaluated_byte_count: roundtrip.validation.evaluated_byte_count,
        }
    } else if let Some(final_validation) = source_report.final_validation.as_ref() {
        ParameterGolfHomegolfDenseBaselineMetric {
            metric_source: String::from("final_validation_live_model"),
            val_loss: final_validation.mean_loss,
            val_bpb: final_validation.bits_per_byte,
            evaluated_token_count: final_validation.evaluated_token_count,
            evaluated_byte_count: final_validation.evaluated_byte_count,
        }
    } else {
        return Err(
            ParameterGolfHomegolfDenseBaselineSurfaceError::InvalidSource {
                message: String::from(
                    "source dense baseline report is missing both final_roundtrip_receipt and final_validation",
                ),
            },
        );
    };

    let compressed_model_bytes = source_report.compressed_model_bytes.ok_or_else(|| {
        ParameterGolfHomegolfDenseBaselineSurfaceError::InvalidSource {
            message: String::from("source dense baseline report is missing compressed_model_bytes"),
        }
    })?;
    let compressed_model_artifact_ref = source_report
        .compressed_model_artifact_ref
        .clone()
        .ok_or_else(
            || ParameterGolfHomegolfDenseBaselineSurfaceError::InvalidSource {
                message: String::from(
                    "source dense baseline report is missing compressed_model_artifact_ref",
                ),
            },
        )?;
    let compressed_model_artifact_digest = source_report
        .compressed_model_artifact_digest
        .clone()
        .ok_or_else(|| {
        ParameterGolfHomegolfDenseBaselineSurfaceError::InvalidSource {
            message: String::from(
                "source dense baseline report is missing compressed_model_artifact_digest",
            ),
        }
    })?;

    let mut report = ParameterGolfHomegolfDenseBaselineSurfaceReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_dense_baseline_surface.v1"),
        track_id: String::from("parameter_golf.home_cluster_compatible_10min.v1"),
        strict_profile_id: String::from(PARAMETER_GOLF_PROMOTED_CHALLENGE_PROFILE_ID),
        source_training_report_ref: String::from(
            PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SOURCE_REPORT_REF,
        ),
        source_training_report_digest: source_report.report_digest.clone(),
        trainer_entrypoint: String::from(
            "crates/psionic-train/src/bin/parameter_golf_single_h100_train.rs",
        ),
        trainer_implementation: String::from(
            "crates/psionic-train/src/parameter_golf_single_h100_training.rs",
        ),
        baseline_model_id: String::from(PARAMETER_GOLF_BASELINE_MODEL_ID),
        baseline_revision: String::from(PARAMETER_GOLF_BASELINE_REVISION),
        baseline_config: ParameterGolfConfig::baseline_sp1024_9x512(),
        batch_geometry: source_report.geometry.clone(),
        max_steps: source_report.max_steps,
        executed_steps: source_report.executed_steps,
        stop_reason: source_report.stop_reason,
        train_token_count: source_report.train_token_count,
        observed_wallclock_ms: source_report.observed_wallclock_ms,
        compressed_model_bytes,
        compressed_model_artifact_ref,
        compressed_model_artifact_digest,
        final_metric,
        claim_boundary: String::from(
            "This freezes the first honest HOMEGOLF dense baseline surface: Psionic already executes the exact dense 9x512 SP1024 trainer and emits real validation, wallclock, token-count, and artifact-size outputs. It does not yet claim a 10-minute mixed-device home-cluster score or public-leaderboard-equivalent hardware.",
        ),
        summary: String::from(
            "The exact dense PGOLF naive baseline already exists as a real Psionic trainer surface. This HOMEGOLF report binds that retained single-device dense run into the home-cluster track as the first honest baseline surface to improve from.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = stable_digest(
        b"psionic_parameter_golf_homegolf_dense_baseline_surface|",
        &report,
    );
    Ok(report)
}

/// Writes the canonical HOMEGOLF dense-baseline surface report to disk.
pub fn write_parameter_golf_homegolf_dense_baseline_surface_report(
    output_path: &Path,
) -> Result<
    ParameterGolfHomegolfDenseBaselineSurfaceReport,
    ParameterGolfHomegolfDenseBaselineSurfaceError,
> {
    let report = build_parameter_golf_homegolf_dense_baseline_surface_report()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfDenseBaselineSurfaceError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfDenseBaselineSurfaceError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn parameter_golf_homegolf_dense_baseline_source_report_path() -> PathBuf {
    repo_root().join(PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SOURCE_REPORT_REF)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value).expect("HOMEGOLF dense baseline surface report should serialize"),
    );
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use psionic_models::{PARAMETER_GOLF_BASELINE_MODEL_ID, ParameterGolfConfig};

    use super::{
        PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SURFACE_REPORT_REF,
        build_parameter_golf_homegolf_dense_baseline_surface_report,
        write_parameter_golf_homegolf_dense_baseline_surface_report,
    };

    #[test]
    fn homegolf_dense_baseline_surface_keeps_exact_pgolf_contract_and_metrics() {
        let report = build_parameter_golf_homegolf_dense_baseline_surface_report()
            .expect("HOMEGOLF dense baseline surface should build");
        assert_eq!(
            report.track_id,
            "parameter_golf.home_cluster_compatible_10min.v1"
        );
        assert_eq!(report.baseline_model_id, PARAMETER_GOLF_BASELINE_MODEL_ID);
        assert_eq!(
            report.baseline_config,
            ParameterGolfConfig::baseline_sp1024_9x512()
        );
        assert_eq!(
            report.batch_geometry,
            crate::ParameterGolfBatchGeometry::challenge_single_device_defaults()
        );
        assert_eq!(report.executed_steps, 1);
        assert!(report.train_token_count > 0);
        assert!(report.observed_wallclock_ms > 0);
        assert!(report.compressed_model_bytes > 0);
        assert!(report.final_metric.val_loss > 0.0);
        assert!(report.final_metric.val_bpb > 0.0);
        assert!(
            report
                .claim_boundary
                .contains("does not yet claim a 10-minute mixed-device home-cluster score")
        );
    }

    #[test]
    fn homegolf_dense_baseline_surface_fixture_stays_in_sync() {
        let expected_path =
            repo_root().join(PARAMETER_GOLF_HOMEGOLF_DENSE_BASELINE_SURFACE_REPORT_REF);
        let output_path = PathBuf::from(std::env::temp_dir())
            .join("parameter_golf_homegolf_dense_baseline_surface_fixture_test.json");
        write_parameter_golf_homegolf_dense_baseline_surface_report(&output_path)
            .expect("HOMEGOLF dense baseline surface should write");
        let fixture = std::fs::read_to_string(&expected_path).expect("fixture should exist");
        let generated = std::fs::read_to_string(&output_path).expect("generated file should exist");
        assert_eq!(fixture, generated);
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }
}
