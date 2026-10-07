use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PARAMETER_GOLF_HOMEGOLF_CLUSTERED_RUN_SURFACE_FIXTURE_PATH: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_clustered_run_surface.json";
pub const PARAMETER_GOLF_HOMEGOLF_CLUSTERED_RUN_SURFACE_CHECKER: &str =
    "scripts/check-parameter-golf-homegolf-clustered-run-surface.sh";
pub const PARAMETER_GOLF_HOMEGOLF_CLUSTERED_RUN_SURFACE_AUDIT: &str =
    "docs/audits/2026-03-27-homegolf-live-dense-run-surface.md";

const PARAMETER_GOLF_HOMEGOLF_TRACK_CONTRACT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_homegolf_track_contract.json";
const FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_REF: &str =
    "fixtures/training/first_same_job_mixed_backend_dense_run_v1.json";
const PARAMETER_GOLF_SINGLE_H100_TRAINING_REPORT_REF: &str =
    "fixtures/parameter_golf/reports/parameter_golf_runpod_single_h100_first_real_training_report.json";
const HOMEGOLF_TRACK_ID: &str = "parameter_golf.home_cluster_compatible_10min.v1";

#[derive(Debug, Error)]
pub enum ParameterGolfHomegolfClusteredRunSurfaceError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("invalid clustered HOMEGOLF surface: {detail}")]
    InvalidSurface { detail: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterGolfHomegolfClusteredRunSurfaceStatus {
    LiveDenseMixedDeviceSurface,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfClusteredRunContribution {
    pub node_id: String,
    pub runtime_role: String,
    pub role_id: String,
    pub execution_backend_label: String,
    pub endpoint: String,
    pub observed_wallclock_ms: u64,
    pub local_execution_wallclock_ms: u64,
    pub executed_steps: u64,
    pub batch_count: u64,
    pub sample_count: u64,
    pub payload_bytes: u64,
    pub final_mean_loss: f64,
    pub contributor_receipt_digest: String,
    pub estimated_steps_per_second: f64,
    pub estimated_samples_per_second: f64,
    pub contribution_share: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfHomegolfClusteredRunSurfaceReport {
    pub schema_version: u16,
    pub report_id: String,
    pub track_id: String,
    pub run_id: String,
    pub wallclock_cap_seconds: u64,
    pub observed_cluster_wallclock_ms: u64,
    pub source_track_contract_ref: String,
    pub source_mixed_backend_dense_run_ref: String,
    pub source_dense_score_report_ref: String,
    pub admitted_device_set: Vec<String>,
    pub per_device_contributions: Vec<ParameterGolfHomegolfClusteredRunContribution>,
    pub merge_disposition: String,
    pub publish_disposition: String,
    pub promotion_disposition: String,
    pub merged_bundle_descriptor_digest: String,
    pub merged_bundle_tokenizer_digest: String,
    pub scored_model_artifact_ref: String,
    pub scored_model_artifact_digest: String,
    pub final_validation_mean_loss: f64,
    pub final_validation_bits_per_byte: f64,
    pub model_artifact_bytes: u64,
    pub surface_status: ParameterGolfHomegolfClusteredRunSurfaceStatus,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MixedBackendDenseRunSource {
    run_id: String,
    world_size: u16,
    participants: Vec<MixedBackendDenseParticipantSource>,
    source_bindings: Vec<MixedBackendDenseSourceBinding>,
    step_metrics: Vec<MixedBackendDenseStepMetricSource>,
    final_disposition: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MixedBackendDenseParticipantSource {
    participant_id: String,
    source_id: String,
    backend_family: String,
    runtime_family_id: String,
    logical_rank_count: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct MixedBackendDenseSourceBinding {
    source_id: String,
    source_contract_digest: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MixedBackendDenseStepMetricSource {
    mean_train_loss: String,
    train_tokens: u64,
    cuda_submesh_step_ms: u64,
    mlx_rank_step_ms: u64,
    cross_backend_bridge_ms: u64,
    optimizer_step_ms: u64,
}

#[derive(Debug, Deserialize)]
struct DenseScoreReportSource {
    run_id: String,
    tokenizer_digest: DenseScoreTokenizerDigest,
    baseline_model_descriptor_digest: String,
    final_validation: Option<DenseScoreValidationSummary>,
    final_roundtrip_receipt: Option<DenseScoreRoundtripReceipt>,
    compressed_model_bytes: Option<u64>,
    compressed_model_artifact_ref: Option<String>,
    compressed_model_artifact_digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DenseScoreTokenizerDigest {
    tokenizer_digest: String,
}

#[derive(Debug, Deserialize)]
struct DenseScoreRoundtripReceipt {
    validation: DenseScoreValidationSummary,
}

#[derive(Clone, Debug, Deserialize)]
struct DenseScoreValidationSummary {
    mean_loss: f64,
    bits_per_byte: f64,
}

impl ParameterGolfHomegolfClusteredRunSurfaceReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(
            b"psionic_parameter_golf_homegolf_clustered_run_surface|",
            &clone,
        )
    }

    pub fn validate(&self) -> Result<(), ParameterGolfHomegolfClusteredRunSurfaceError> {
        if self.schema_version != 1 {
            return Err(
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                    detail: format!("schema_version must stay 1 but was {}", self.schema_version),
                },
            );
        }
        if self.track_id != HOMEGOLF_TRACK_ID {
            return Err(
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                    detail: String::from("track_id drifted"),
                },
            );
        }
        if self.wallclock_cap_seconds != 600 {
            return Err(
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                    detail: String::from("wallclock_cap_seconds must stay 600"),
                },
            );
        }
        if self.observed_cluster_wallclock_ms == 0
            || self.observed_cluster_wallclock_ms > self.wallclock_cap_seconds * 1_000
        {
            return Err(
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                    detail: String::from(
                        "observed_cluster_wallclock_ms must stay positive and within the 600s cap",
                    ),
                },
            );
        }
        if self.admitted_device_set
            != vec![
                String::from("local_apple_silicon_metal"),
                String::from("optional_h100_node"),
            ]
        {
            return Err(ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                detail: String::from(
                    "admitted_device_set must retain the current mixed-device HOMEGOLF live surface classes",
                ),
            });
        }
        if self.per_device_contributions.len() != 2 {
            return Err(ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                detail: String::from(
                    "per_device_contributions must retain exactly two mixed-device dense receipts",
                ),
            });
        }
        if self.merge_disposition != "merged"
            || self.publish_disposition != "held"
            || self.promotion_disposition != "held"
        {
            return Err(
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                    detail: String::from("merge, publish, and promotion dispositions drifted"),
                },
            );
        }
        if self.model_artifact_bytes == 0
            || self.final_validation_bits_per_byte <= 0.0
            || self.final_validation_mean_loss <= 0.0
        {
            return Err(
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                    detail: String::from(
                        "model_artifact_bytes and final validation metrics must stay positive",
                    ),
                },
            );
        }
        if self.surface_status
            != ParameterGolfHomegolfClusteredRunSurfaceStatus::LiveDenseMixedDeviceSurface
        {
            return Err(
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                    detail: String::from("surface_status drifted"),
                },
            );
        }
        if self.report_digest != self.stable_digest() {
            return Err(
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                    detail: String::from("report_digest drifted"),
                },
            );
        }
        Ok(())
    }
}

pub fn build_parameter_golf_homegolf_clustered_run_surface_report() -> Result<
    ParameterGolfHomegolfClusteredRunSurfaceReport,
    ParameterGolfHomegolfClusteredRunSurfaceError,
> {
    let mixed_backend_run: MixedBackendDenseRunSource = serde_json::from_slice(
        &fs::read(resolve_repo_path(
            FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_REF,
        ))
        .map_err(
            |error| ParameterGolfHomegolfClusteredRunSurfaceError::Read {
                path: String::from(FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_REF),
                error,
            },
        )?,
    )?;
    let dense_score_report: DenseScoreReportSource = serde_json::from_slice(
        &fs::read(resolve_repo_path(
            PARAMETER_GOLF_SINGLE_H100_TRAINING_REPORT_REF,
        ))
        .map_err(
            |error| ParameterGolfHomegolfClusteredRunSurfaceError::Read {
                path: String::from(PARAMETER_GOLF_SINGLE_H100_TRAINING_REPORT_REF),
                error,
            },
        )?,
    )?;

    let step_count = mixed_backend_run.step_metrics.len() as u64;
    if step_count == 0 {
        return Err(
            ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                detail: String::from("mixed-backend dense run retained zero steps"),
            },
        );
    }
    let total_train_tokens = mixed_backend_run
        .step_metrics
        .iter()
        .map(|metric| metric.train_tokens)
        .sum::<u64>();
    let observed_cluster_wallclock_ms = mixed_backend_run
        .step_metrics
        .iter()
        .map(|metric| {
            metric
                .cuda_submesh_step_ms
                .max(metric.mlx_rank_step_ms)
                .saturating_add(metric.cross_backend_bridge_ms)
                .saturating_add(metric.optimizer_step_ms)
        })
        .sum::<u64>();
    let final_mean_loss = mixed_backend_run
        .step_metrics
        .last()
        .ok_or_else(
            || ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                detail: String::from("mixed-backend dense run missing final step"),
            },
        )?
        .mean_train_loss
        .parse::<f64>()
        .map_err(
            |error| ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                detail: format!("failed to parse final mean_train_loss: {error}"),
            },
        )?;

    let source_digest_by_source_id = mixed_backend_run
        .source_bindings
        .iter()
        .map(|binding| {
            (
                binding.source_id.clone(),
                binding.source_contract_digest.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let per_device_contributions = mixed_backend_run
        .participants
        .iter()
        .map(|participant| {
            let local_execution_wallclock_ms = match participant.backend_family.as_str() {
                "cuda" => mixed_backend_run
                    .step_metrics
                    .iter()
                    .map(|metric| metric.cuda_submesh_step_ms)
                    .sum::<u64>(),
                "mlx_metal" => mixed_backend_run
                    .step_metrics
                    .iter()
                    .map(|metric| metric.mlx_rank_step_ms)
                    .sum::<u64>(),
                other => {
                    return Err(
                        ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                            detail: format!("unsupported backend family `{other}`"),
                        },
                    )
                }
            };
            let estimated_steps_per_second =
                step_count as f64 / ((local_execution_wallclock_ms as f64) / 1_000.0);
            let estimated_samples_per_second =
                total_train_tokens as f64 / ((local_execution_wallclock_ms as f64) / 1_000.0);
            let contribution_share =
                f64::from(participant.logical_rank_count) / f64::from(mixed_backend_run.world_size);
            Ok(ParameterGolfHomegolfClusteredRunContribution {
                node_id: participant.participant_id.clone(),
                runtime_role: String::from("dense_full_model_rank"),
                role_id: participant.runtime_family_id.clone(),
                execution_backend_label: participant.backend_family.clone(),
                endpoint: participant.source_id.clone(),
                observed_wallclock_ms: observed_cluster_wallclock_ms,
                local_execution_wallclock_ms,
                executed_steps: step_count,
                batch_count: step_count,
                sample_count: total_train_tokens,
                payload_bytes: 0,
                final_mean_loss,
                contributor_receipt_digest: source_digest_by_source_id
                    .get(participant.source_id.as_str())
                    .cloned()
                    .unwrap_or_else(|| stable_digest(b"missing_source_digest|", participant)),
                estimated_steps_per_second,
                estimated_samples_per_second,
                contribution_share,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    let final_validation =
        dense_score_report
            .final_roundtrip_receipt
            .as_ref()
            .map(|receipt| receipt.validation.clone())
            .or_else(|| dense_score_report.final_validation.clone())
            .ok_or_else(|| {
                ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
            detail: String::from(
                "dense score report is missing both final_roundtrip_receipt and final_validation",
            ),
        }
            })?;
    let scored_model_artifact_ref = dense_score_report
        .compressed_model_artifact_ref
        .clone()
        .ok_or_else(
            || ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                detail: String::from("dense score report missing compressed_model_artifact_ref"),
            },
        )?;
    let scored_model_artifact_digest = dense_score_report
        .compressed_model_artifact_digest
        .clone()
        .ok_or_else(
            || ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
                detail: String::from("dense score report missing compressed_model_artifact_digest"),
            },
        )?;
    let model_artifact_bytes = dense_score_report.compressed_model_bytes.ok_or_else(|| {
        ParameterGolfHomegolfClusteredRunSurfaceError::InvalidSurface {
            detail: String::from("dense score report missing compressed_model_bytes"),
        }
    })?;

    let mut report = ParameterGolfHomegolfClusteredRunSurfaceReport {
        schema_version: 1,
        report_id: String::from("parameter_golf.homegolf_clustered_run_surface.v1"),
        track_id: String::from(HOMEGOLF_TRACK_ID),
        run_id: format!(
            "{}+{}",
            mixed_backend_run.run_id, dense_score_report.run_id
        ),
        wallclock_cap_seconds: 600,
        observed_cluster_wallclock_ms,
        source_track_contract_ref: String::from(PARAMETER_GOLF_HOMEGOLF_TRACK_CONTRACT_REF),
        source_mixed_backend_dense_run_ref: String::from(FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_REF),
        source_dense_score_report_ref: String::from(PARAMETER_GOLF_SINGLE_H100_TRAINING_REPORT_REF),
        admitted_device_set: vec![
            String::from("local_apple_silicon_metal"),
            String::from("optional_h100_node"),
        ],
        per_device_contributions,
        merge_disposition: String::from("merged"),
        publish_disposition: String::from("held"),
        promotion_disposition: String::from("held"),
        merged_bundle_descriptor_digest: dense_score_report.baseline_model_descriptor_digest,
        merged_bundle_tokenizer_digest: dense_score_report.tokenizer_digest.tokenizer_digest,
        scored_model_artifact_ref,
        scored_model_artifact_digest,
        final_validation_mean_loss: final_validation.mean_loss,
        final_validation_bits_per_byte: final_validation.bits_per_byte,
        model_artifact_bytes,
        surface_status: ParameterGolfHomegolfClusteredRunSurfaceStatus::LiveDenseMixedDeviceSurface,
        claim_boundary: String::from(
            "This retained HOMEGOLF surface replaces the older open-adapter-plus-bounded-bundle surrogate with dense retained sources only: one real same-job MLX-plus-CUDA dense runtime proof and one real exact dense challenge export carrying the scored compressed model artifact and contest-style final roundtrip metric. The retained mixed-device runtime source here is still the local MLX rank plus the optional-H100 CUDA submesh, not the currently reachable Apple-plus-home-RTX cluster. It is materially stronger than the earlier composed surrogate, but it still does not claim admitted home-RTX dense closure or a single retained run id that already binds both the mixed-device dense runtime receipts and the final scored export bytes in one artifact family.",
        ),
        summary: String::from(
            "The current retained H100-backed HOMEGOLF surface now binds real mixed-device dense execution truth to the exact dense challenge export surface instead of the older open-adapter composition. The retained report freezes mixed-device dense contribution receipts, dense wallclock, descriptor and tokenizer digests, scored compressed-model bytes, and final contest-style validation metrics in one HOMEGOLF machine-readable surface.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate()?;
    Ok(report)
}

pub fn write_parameter_golf_homegolf_clustered_run_surface_report(
    output_path: &Path,
) -> Result<
    ParameterGolfHomegolfClusteredRunSurfaceReport,
    ParameterGolfHomegolfClusteredRunSurfaceError,
> {
    let report = build_parameter_golf_homegolf_clustered_run_surface_report()?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ParameterGolfHomegolfClusteredRunSurfaceError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(output_path, bytes).map_err(|error| {
        ParameterGolfHomegolfClusteredRunSurfaceError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("clustered HOMEGOLF surface should serialize"));
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
        build_parameter_golf_homegolf_clustered_run_surface_report,
        write_parameter_golf_homegolf_clustered_run_surface_report,
        ParameterGolfHomegolfClusteredRunSurfaceStatus,
        PARAMETER_GOLF_HOMEGOLF_CLUSTERED_RUN_SURFACE_FIXTURE_PATH,
    };

    #[test]
    fn clustered_homegolf_surface_keeps_live_dense_mixed_device_truth() {
        let report =
            build_parameter_golf_homegolf_clustered_run_surface_report().expect("build report");
        assert_eq!(report.wallclock_cap_seconds, 600);
        assert_eq!(
            report.surface_status,
            ParameterGolfHomegolfClusteredRunSurfaceStatus::LiveDenseMixedDeviceSurface
        );
        assert_eq!(report.admitted_device_set.len(), 2);
        assert!(report.observed_cluster_wallclock_ms <= 600_000);
        assert!(report.final_validation_bits_per_byte > 0.0);
        assert!(report.model_artifact_bytes > 0);
        assert!(report.model_artifact_bytes < 16_000_000);
    }

    #[test]
    fn write_clustered_homegolf_surface_roundtrips() {
        let output = tempfile::tempdir().expect("tempdir");
        let path = output
            .path()
            .join("parameter_golf_homegolf_clustered_run_surface.json");
        let written = write_parameter_golf_homegolf_clustered_run_surface_report(path.as_path())
            .expect("write report");
        let encoded = std::fs::read(path.as_path()).expect("read report");
        let decoded: super::ParameterGolfHomegolfClusteredRunSurfaceReport =
            serde_json::from_slice(&encoded).expect("decode report");
        assert_eq!(written, decoded);
    }

    #[test]
    fn committed_clustered_homegolf_surface_fixture_roundtrips() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join(PARAMETER_GOLF_HOMEGOLF_CLUSTERED_RUN_SURFACE_FIXTURE_PATH);
        let encoded = std::fs::read(fixture).expect("read fixture");
        let decoded: super::ParameterGolfHomegolfClusteredRunSurfaceReport =
            serde_json::from_slice(&encoded).expect("decode fixture");
        let rebuilt =
            build_parameter_golf_homegolf_clustered_run_surface_report().expect("rebuild report");
        assert_eq!(decoded, rebuilt);
    }
}
