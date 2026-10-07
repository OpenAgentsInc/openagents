use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use psionic_eval::{
    ParameterGolfDistributedLaneDisposition, ParameterGolfDistributedThroughputReceipt,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    build_remote_training_run_index, record_remote_training_visualization_bundle,
    ParameterGolfDistributed8xH100TrainStepReceipt, RemoteTrainingArtifactSourceKind,
    RemoteTrainingDistributedSample, RemoteTrainingEmissionMode, RemoteTrainingEventSample,
    RemoteTrainingEventSeverity, RemoteTrainingGpuSample, RemoteTrainingHeartbeatSample,
    RemoteTrainingLossSample, RemoteTrainingMathSample, RemoteTrainingProvider,
    RemoteTrainingRefreshContract, RemoteTrainingResultClassification, RemoteTrainingRunIndex,
    RemoteTrainingRunIndexEntry, RemoteTrainingRuntimeSample, RemoteTrainingSeriesStatus,
    RemoteTrainingSourceArtifact, RemoteTrainingTimelineEntry, RemoteTrainingVisualizationBundle,
    RemoteTrainingVisualizationError, RemoteTrainingVisualizationSummary,
    REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
};

pub const PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_DIR_NAME: &str = "training_visualization";
pub const PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_BUNDLE_NAME: &str =
    "parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v1.json";
pub const PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_RUN_INDEX_NAME: &str =
    "remote_training_run_index_v1.json";
pub const PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_SNAPSHOT_DIR_NAME: &str = "snapshots";
const PARAMETER_GOLF_DISTRIBUTED_LIVE_STALE_AFTER_MS: u64 = 2_500;
const REMOTE_TRAINING_PROFILE_ID_ENV: &str = "PSIONIC_REMOTE_TRAINING_PROFILE_ID";
const REMOTE_TRAINING_LANE_ID_ENV: &str = "PSIONIC_REMOTE_TRAINING_LANE_ID";
const REMOTE_TRAINING_REPO_REVISION_ENV: &str = "PSIONIC_REMOTE_TRAINING_REPO_REVISION";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterGolfDistributedVisualizationMetadata {
    pub provider: RemoteTrainingProvider,
    pub profile_id: String,
    pub lane_id: String,
    pub repo_revision: String,
    pub run_root: PathBuf,
    pub bringup_report_path: PathBuf,
    pub runtime_manifest_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
struct ParameterGolfDistributedVisualizationState {
    run_id: String,
    result_classification: RemoteTrainingResultClassification,
    started_at_ms: u64,
    finished_at_ms: Option<u64>,
    phase: String,
    subphase: Option<String>,
    step_in_progress: Option<u64>,
    active_subsystems: Vec<String>,
    summary_detail: String,
    heartbeat_seq: u64,
    total_steps_completed: u64,
    latest_checkpoint_ref: Option<String>,
    timeline: Vec<RemoteTrainingTimelineEntry>,
    heartbeat_series: Vec<RemoteTrainingHeartbeatSample>,
    loss_series: Vec<RemoteTrainingLossSample>,
    math_series: Vec<RemoteTrainingMathSample>,
    runtime_series: Vec<RemoteTrainingRuntimeSample>,
    gpu_series: Vec<RemoteTrainingGpuSample>,
    distributed_series: Vec<RemoteTrainingDistributedSample>,
    event_series: Vec<RemoteTrainingEventSample>,
    source_artifacts: BTreeMap<String, RemoteTrainingSourceArtifact>,
}

struct ParameterGolfDistributedVisualizationShared {
    state: ParameterGolfDistributedVisualizationState,
    last_flush_at_ms: Option<u64>,
    background_error: Option<String>,
}

pub struct ParameterGolfDistributedLiveVisualizationWriter {
    metadata: ParameterGolfDistributedVisualizationMetadata,
    paths: ParameterGolfDistributedVisualizationPaths,
    shared: Arc<Mutex<ParameterGolfDistributedVisualizationShared>>,
    stop_requested: Arc<AtomicBool>,
    background_thread: Option<JoinHandle<()>>,
}

#[derive(Debug, Error)]
pub enum ParameterGolfDistributedVisualizationError {
    #[error("parameter golf distributed visualization could not read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("parameter golf distributed visualization could not write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("parameter golf distributed visualization could not decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("parameter golf distributed visualization could not encode `{path}`: {error}")]
    Serialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("parameter golf distributed live visualization background worker failed: {message}")]
    Background { message: String },
    #[error(transparent)]
    Contract(#[from] RemoteTrainingVisualizationError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRunPod8xH100FinalizerReport {
    pub schema_version: String,
    pub runner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at_utc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trainer_lane_id: Option<String>,
    pub run_root: String,
    pub submission_dir: String,
    pub world_size: u64,
    pub grad_accum_steps: u64,
    pub accelerator_evidence: ParameterGolfRunPod8xH100AcceleratorEvidence,
    pub exported_folder_evidence: ParameterGolfRunPod8xH100ExportedFolderEvidence,
    pub claim_boundary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRunPod8xH100AcceleratorEvidence {
    pub inventory_path: String,
    pub topology_path: String,
    pub inventory_line_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfRunPod8xH100ExportedFolderEvidence {
    pub entrypoint_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint_sha256: Option<String>,
    pub submission_manifest_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_run_evidence_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submission_run_evidence_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_receipt_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distributed_receipt_sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterGolfDistributedVisualizationPaths {
    pub bundle_path: PathBuf,
    pub run_index_path: PathBuf,
    pub snapshot_dir: PathBuf,
    pub finalized_snapshot_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterGolfDistributedVisualizationWriteOutcome {
    pub bundle: RemoteTrainingVisualizationBundle,
    pub run_index: RemoteTrainingRunIndex,
    pub paths: ParameterGolfDistributedVisualizationPaths,
}

impl ParameterGolfDistributedVisualizationMetadata {
    #[must_use]
    pub fn new_for_runpod_runtime(
        run_root: impl Into<PathBuf>,
        bringup_report_path: impl Into<PathBuf>,
        runtime_manifest_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            provider: RemoteTrainingProvider::RunPod,
            profile_id: env::var(REMOTE_TRAINING_PROFILE_ID_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| String::from("runpod_8xh100_parameter_golf")),
            lane_id: env::var(REMOTE_TRAINING_LANE_ID_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| String::from("parameter_golf_distributed_8xh100")),
            repo_revision: env::var(REMOTE_TRAINING_REPO_REVISION_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| String::from("workspace@unknown")),
            run_root: run_root.into(),
            bringup_report_path: bringup_report_path.into(),
            runtime_manifest_path: runtime_manifest_path.into(),
        }
    }

    #[must_use]
    pub fn paths(&self) -> ParameterGolfDistributedVisualizationPaths {
        visualization_paths(self.run_root.as_path())
    }

    #[must_use]
    pub fn bundle_relative_path(&self) -> String {
        format!(
            "{}/{}",
            PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_DIR_NAME,
            PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_BUNDLE_NAME
        )
    }

    #[must_use]
    pub fn run_index_relative_path(&self) -> String {
        format!(
            "{}/{}",
            PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_DIR_NAME,
            PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_RUN_INDEX_NAME
        )
    }
}

impl ParameterGolfDistributedLiveVisualizationWriter {
    pub fn start(
        metadata: ParameterGolfDistributedVisualizationMetadata,
        run_id: &str,
        initial_detail: impl Into<String>,
    ) -> Result<Self, ParameterGolfDistributedVisualizationError> {
        let paths = metadata.paths();
        fs::create_dir_all(
            paths
                .bundle_path
                .parent()
                .expect("bundle path should have a parent"),
        )
        .map_err(|error| ParameterGolfDistributedVisualizationError::Write {
            path: metadata.run_root.display().to_string(),
            error,
        })?;
        fs::create_dir_all(paths.snapshot_dir.as_path()).map_err(|error| {
            ParameterGolfDistributedVisualizationError::Write {
                path: paths.snapshot_dir.display().to_string(),
                error,
            }
        })?;
        let started_at_ms = unix_time_ms();
        let initial_detail = initial_detail.into();
        let bringup_relative_path = relative_path_string(
            metadata.run_root.as_path(),
            metadata.bringup_report_path.as_path(),
        )
        .unwrap_or_else(|| metadata.bringup_report_path.display().to_string());
        let manifest_relative_path = relative_path_string(
            metadata.run_root.as_path(),
            metadata.runtime_manifest_path.as_path(),
        )
        .unwrap_or_else(|| metadata.runtime_manifest_path.display().to_string());
        let mut source_artifacts = BTreeMap::new();
        source_artifacts.insert(
            String::from("live_bundle"),
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("live_bundle"),
                artifact_uri: metadata.bundle_relative_path(),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf_distributed_8xh100.remote_training_bundle.v1",
                )],
                detail: String::from(
                    "The provider-neutral local mirror is the app-facing source for the RunPod distributed 8xH100 lane.",
                ),
            },
        );
        source_artifacts.insert(
            String::from("bringup_report"),
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("bringup_report"),
                artifact_uri: bringup_relative_path,
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf_distributed_8xh100_bringup_report",
                )],
                detail: String::from(
                    "The Rust-owned bring-up report remains authoritative for exact machine admission on the distributed lane.",
                ),
            },
        );
        source_artifacts.insert(
            String::from("runtime_manifest"),
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("runtime_manifest"),
                artifact_uri: manifest_relative_path,
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf_submission_runtime_manifest",
                )],
                detail: String::from(
                    "The shipped runtime manifest remains authoritative for the exported-folder runtime contract.",
                ),
            },
        );
        let shared = Arc::new(Mutex::new(ParameterGolfDistributedVisualizationShared {
            state: ParameterGolfDistributedVisualizationState {
                run_id: run_id.to_string(),
                result_classification: RemoteTrainingResultClassification::Active,
                started_at_ms,
                finished_at_ms: None,
                phase: String::from("training"),
                subphase: Some(String::from("runtime_bootstrap")),
                step_in_progress: Some(1),
                active_subsystems: vec![
                    String::from("runtime_bootstrap"),
                    String::from("rank_fanout"),
                    String::from("training_visualization"),
                ],
                summary_detail: initial_detail.clone(),
                heartbeat_seq: 0,
                total_steps_completed: 0,
                latest_checkpoint_ref: None,
                timeline: vec![RemoteTrainingTimelineEntry {
                    observed_at_ms: started_at_ms,
                    phase: String::from("training"),
                    subphase: Some(String::from("runtime_bootstrap")),
                    detail: initial_detail.clone(),
                }],
                heartbeat_series: Vec::new(),
                loss_series: Vec::new(),
                math_series: Vec::new(),
                runtime_series: Vec::new(),
                gpu_series: Vec::new(),
                distributed_series: Vec::new(),
                event_series: vec![RemoteTrainingEventSample {
                    observed_at_ms: started_at_ms,
                    severity: RemoteTrainingEventSeverity::Info,
                    event_kind: String::from("runtime_started"),
                    detail: initial_detail,
                }],
                source_artifacts,
            },
            last_flush_at_ms: None,
            background_error: None,
        }));
        let stop_requested = Arc::new(AtomicBool::new(false));
        let mut writer = Self {
            metadata,
            paths,
            shared: Arc::clone(&shared),
            stop_requested: Arc::clone(&stop_requested),
            background_thread: None,
        };
        writer.flush(true)?;
        writer.background_thread = Some(spawn_background_writer(
            writer.metadata.clone(),
            writer.paths.clone(),
            shared,
            stop_requested,
        ));
        Ok(writer)
    }

    pub fn record_phase(
        &mut self,
        phase: impl Into<String>,
        subphase: Option<String>,
        detail: impl Into<String>,
        active_subsystems: Vec<String>,
        step_in_progress: Option<u64>,
        force_flush: bool,
    ) -> Result<(), ParameterGolfDistributedVisualizationError> {
        self.with_shared_mut(|shared| {
            let observed_at_ms = unix_time_ms();
            let phase = phase.into();
            let detail = detail.into();
            let timeline_changed = shared.state.phase != phase || shared.state.subphase != subphase;
            shared.state.phase = phase.clone();
            shared.state.subphase = subphase.clone();
            shared.state.step_in_progress = step_in_progress;
            shared.state.active_subsystems = dedup_strings(active_subsystems);
            shared.state.summary_detail = detail.clone();
            if timeline_changed {
                shared.state.timeline.push(RemoteTrainingTimelineEntry {
                    observed_at_ms,
                    phase,
                    subphase,
                    detail,
                });
            }
            Ok(())
        })?;
        if force_flush {
            self.flush(true)?;
        }
        Ok(())
    }

    pub fn record_event(
        &mut self,
        severity: RemoteTrainingEventSeverity,
        event_kind: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<(), ParameterGolfDistributedVisualizationError> {
        self.with_shared_mut(|shared| {
            shared.state.event_series.push(RemoteTrainingEventSample {
                observed_at_ms: unix_time_ms(),
                severity,
                event_kind: event_kind.into(),
                detail: detail.into(),
            });
            Ok(())
        })
    }

    pub fn record_train_step_receipt(
        &mut self,
        receipt: &ParameterGolfDistributed8xH100TrainStepReceipt,
    ) -> Result<(), ParameterGolfDistributedVisualizationError> {
        let run_root = self.metadata.run_root.clone();
        self.with_shared_mut(|shared| {
            let observed_at_ms = unix_time_ms();
            let validation_loss = receipt
                .distributed_receipt
                .validation_aggregation
                .as_ref()
                .map(|validation| validation.mean_loss as f32);
            let slowest_rank_ms = receipt
                .rank_launches
                .iter()
                .filter_map(|launch| launch.receipt.as_ref().map(|child| child.observed_wallclock_ms))
                .max();
            let fastest_rank_ms = receipt
                .rank_launches
                .iter()
                .filter_map(|launch| launch.receipt.as_ref().map(|child| child.observed_wallclock_ms))
                .min();
            let rank_skew_ms = match (slowest_rank_ms, fastest_rank_ms) {
                (Some(slowest), Some(fastest)) => Some(slowest.saturating_sub(fastest)),
                _ => None,
            };
            let mean_forward_ms = mean_rank_timing_ms(receipt, |child| {
                child.phase_timings.forward_loss_cuda_ms
            });
            let mean_backward_ms = mean_rank_timing_ms(receipt, |child| {
                child.phase_timings.backward_cuda_ms
            });
            shared.state.total_steps_completed =
                shared.state.total_steps_completed.max(receipt.step_observation.global_step);
            shared.state.latest_checkpoint_ref = Some(relative_path_string(
                run_root.as_path(),
                Path::new(receipt.current_model_int8_zlib_artifact_path.as_str()),
            )
            .unwrap_or_else(|| receipt.current_model_int8_zlib_artifact_path.clone()));
            shared.state.summary_detail = format!(
                "The RunPod distributed lane completed optimizer step {} and retained distributed validation across {} ranks.",
                receipt.step_observation.global_step, receipt.world_size
            );
            shared.state.loss_series.push(RemoteTrainingLossSample {
                global_step: Some(receipt.step_observation.global_step),
                elapsed_ms: observed_at_ms.saturating_sub(shared.state.started_at_ms),
                train_loss: Some(receipt.mean_train_loss),
                ema_loss: None,
                validation_loss,
            });
            shared.state.math_series.push(RemoteTrainingMathSample {
                observed_at_ms,
                global_step: Some(receipt.step_observation.global_step),
                learning_rate: None,
                gradient_norm: Some(receipt.gradient_norm_after_clip),
                parameter_norm: None,
                update_norm: None,
                clip_fraction: None,
                clip_event_count: Some(u32::from(receipt.clip_applied)),
                loss_scale: None,
                non_finite_count: receipt.non_finite_gradient_count.min(u64::from(u32::MAX)) as u32,
                model_specific_diagnostics: BTreeMap::from([
                    (String::from("gradient_sync_ms"), receipt.gradient_sync_ms as f32),
                    (String::from("optimizer_step_ms"), receipt.optimizer_step_ms as f32),
                    (
                        String::from("validation_sequence_count"),
                        receipt.validation_total_sequence_count as f32,
                    ),
                ]),
            });
            shared.state.runtime_series.push(RemoteTrainingRuntimeSample {
                observed_at_ms,
                data_wait_ms: None,
                forward_ms: mean_forward_ms,
                backward_ms: mean_backward_ms,
                optimizer_ms: Some(receipt.optimizer_step_ms),
                checkpoint_ms: None,
                evaluation_ms: Some(receipt.validation_observed_ms),
                tokens_per_second: Some(tokens_per_second(
                    receipt.train_tokens,
                    receipt.observed_step_ms,
                )),
                samples_per_second_milli: None,
            });
            shared.state.distributed_series.push(RemoteTrainingDistributedSample {
                observed_at_ms,
                participating_rank_count: receipt.world_size.min(usize::from(u16::MAX)) as u16,
                rank_skew_ms,
                slowest_rank_ms,
                collective_ms: Some(receipt.gradient_sync_ms),
                stalled_rank_count: 0,
            });
            shared.state.source_artifacts.insert(
                String::from("runtime_bootstrap_receipt"),
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("runtime_bootstrap_receipt"),
                    artifact_uri: relative_path_string(
                        run_root.as_path(),
                        Path::new(receipt.runtime_bootstrap_receipt_path.as_str()),
                    )
                    .unwrap_or_else(|| receipt.runtime_bootstrap_receipt_path.clone()),
                    artifact_digest: Some(receipt.runtime_bootstrap_receipt_digest.clone()),
                    source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                    authoritative: true,
                    source_receipt_ids: vec![String::from(
                        "parameter_golf_distributed_8xh100_runtime_bootstrap_receipt",
                    )],
                    detail: String::from(
                        "The aggregate runtime-bootstrap receipt remains authoritative for the distributed mesh admission boundary.",
                    ),
                },
            );
            shared.state.source_artifacts.insert(
                String::from("train_step_receipt"),
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("train_step_receipt"),
                    artifact_uri: relative_path_string(
                        run_root.as_path(),
                        Path::new(receipt.train_step_receipt_path.as_str()),
                    )
                    .unwrap_or_else(|| receipt.train_step_receipt_path.clone()),
                    artifact_digest: Some(receipt.receipt_digest.clone()),
                    source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                    authoritative: true,
                    source_receipt_ids: vec![String::from(
                        "parameter_golf_distributed_8xh100_train_step_receipt",
                    )],
                    detail: String::from(
                        "The aggregate train-step receipt remains authoritative for the retained distributed step and validation series.",
                    ),
                },
            );
            shared.state.source_artifacts.insert(
                String::from("distributed_receipt"),
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("distributed_receipt"),
                    artifact_uri: relative_path_string(
                        run_root.as_path(),
                        Path::new(receipt.distributed_receipt_path.as_str()),
                    )
                    .unwrap_or_else(|| receipt.distributed_receipt_path.clone()),
                    artifact_digest: Some(receipt.distributed_receipt.receipt_digest.clone()),
                    source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                    authoritative: true,
                    source_receipt_ids: vec![String::from(
                        "parameter_golf_distributed_throughput_receipt",
                    )],
                    detail: String::from(
                        "The distributed throughput receipt remains authoritative for the retained distributed topology, timing, and validation posture.",
                    ),
                },
            );
            shared.state.source_artifacts.insert(
                String::from("current_model_artifact"),
                RemoteTrainingSourceArtifact {
                    artifact_role: String::from("current_model_artifact"),
                    artifact_uri: relative_path_string(
                        run_root.as_path(),
                        Path::new(receipt.current_model_int8_zlib_artifact_path.as_str()),
                    )
                    .unwrap_or_else(|| receipt.current_model_int8_zlib_artifact_path.clone()),
                    artifact_digest: Some(receipt.current_model_int8_zlib_artifact_sha256.clone()),
                    source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                    authoritative: true,
                    source_receipt_ids: vec![String::from(
                        "parameter_golf_distributed_8xh100_train_step_receipt",
                    )],
                    detail: String::from(
                        "The retained post-step int8+zlib artifact remains authoritative for the current distributed runtime step output.",
                    ),
                },
            );
            Ok(())
        })?;
        self.flush(true)
    }

    pub fn finish(
        &mut self,
        result_classification: RemoteTrainingResultClassification,
        detail: impl Into<String>,
    ) -> Result<
        ParameterGolfDistributedVisualizationWriteOutcome,
        ParameterGolfDistributedVisualizationError,
    > {
        self.with_shared_mut(|shared| {
            let observed_at_ms = unix_time_ms();
            let detail = detail.into();
            shared.state.result_classification = result_classification;
            shared.state.finished_at_ms = Some(observed_at_ms);
            shared.state.phase = String::from("complete");
            shared.state.subphase = Some(match result_classification {
                RemoteTrainingResultClassification::CompletedSuccess => {
                    String::from("runtime_completed")
                }
                RemoteTrainingResultClassification::Refused => String::from("runtime_refused"),
                RemoteTrainingResultClassification::CompletedFailure => {
                    String::from("runtime_failed")
                }
                _ => String::from("runtime_finished"),
            });
            shared.state.step_in_progress = None;
            shared.state.active_subsystems = vec![
                String::from("runtime"),
                String::from("artifact_seal"),
                String::from("training_visualization"),
            ];
            shared.state.summary_detail = detail.clone();
            shared.state.timeline.push(RemoteTrainingTimelineEntry {
                observed_at_ms,
                phase: shared.state.phase.clone(),
                subphase: shared.state.subphase.clone(),
                detail: detail.clone(),
            });
            shared.state.event_series.push(RemoteTrainingEventSample {
                observed_at_ms,
                severity: if result_classification
                    == RemoteTrainingResultClassification::CompletedSuccess
                {
                    RemoteTrainingEventSeverity::Info
                } else {
                    RemoteTrainingEventSeverity::Warning
                },
                event_kind: String::from("runtime_finished"),
                detail,
            });
            Ok(())
        })?;
        self.flush(true)?;
        self.current_outcome()
    }

    fn current_outcome(
        &self,
    ) -> Result<
        ParameterGolfDistributedVisualizationWriteOutcome,
        ParameterGolfDistributedVisualizationError,
    > {
        let bundle =
            read_visualization_bundle(self.paths.bundle_path.as_path())?.ok_or_else(|| {
                ParameterGolfDistributedVisualizationError::Background {
                    message: String::from("distributed live writer did not persist its bundle"),
                }
            })?;
        let run_index = read_run_index(self.paths.run_index_path.as_path())?.ok_or_else(|| {
            ParameterGolfDistributedVisualizationError::Background {
                message: String::from("distributed live writer did not persist its run index"),
            }
        })?;
        Ok(ParameterGolfDistributedVisualizationWriteOutcome {
            bundle,
            run_index,
            paths: self.paths.clone(),
        })
    }

    fn flush(&mut self, force: bool) -> Result<(), ParameterGolfDistributedVisualizationError> {
        let mut shared = self.shared.lock().expect("writer shared state should lock");
        flush_live_locked(&self.metadata, &self.paths, &mut shared, force)
    }

    fn with_shared_mut<R>(
        &mut self,
        update: impl FnOnce(
            &mut ParameterGolfDistributedVisualizationShared,
        ) -> Result<R, ParameterGolfDistributedVisualizationError>,
    ) -> Result<R, ParameterGolfDistributedVisualizationError> {
        let mut shared = self.shared.lock().expect("writer shared state should lock");
        if let Some(message) = shared.background_error.clone() {
            return Err(ParameterGolfDistributedVisualizationError::Background { message });
        }
        update(&mut shared)
    }
}

impl Drop for ParameterGolfDistributedLiveVisualizationWriter {
    fn drop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Some(handle) = self.background_thread.take() {
            let _ = handle.join();
        }
    }
}

pub fn write_parameter_golf_distributed_visualization_from_finalizer_report(
    report_path: &Path,
    repo_revision: impl Into<String>,
) -> Result<
    ParameterGolfDistributedVisualizationWriteOutcome,
    ParameterGolfDistributedVisualizationError,
> {
    let report = read_finalizer_report(report_path)?;
    let run_root = PathBuf::from(report.run_root.as_str());
    let paths = visualization_paths(run_root.as_path());
    let existing_live_bundle = read_visualization_bundle(paths.bundle_path.as_path())?;
    let receipt = report
        .exported_folder_evidence
        .distributed_receipt_path
        .as_deref()
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .map(|path| read_distributed_receipt(path.as_path()))
        .transpose()?;
    let observed_at_ms = unix_time_ms();
    let gpu_series = read_gpu_inventory(
        PathBuf::from(report.accelerator_evidence.inventory_path.as_str()).as_path(),
        observed_at_ms,
    )?;
    let bundle = build_bundle_from_finalizer_report(
        &report,
        report_path,
        repo_revision.into(),
        observed_at_ms,
        receipt.as_ref(),
        existing_live_bundle.as_ref(),
        gpu_series,
    )?;
    let run_index = build_run_index(&report, &bundle)?;
    fs::create_dir_all(
        paths
            .bundle_path
            .parent()
            .expect("bundle path should have a parent"),
    )
    .map_err(|error| ParameterGolfDistributedVisualizationError::Write {
        path: run_root.display().to_string(),
        error,
    })?;
    write_atomically_json(paths.bundle_path.as_path(), &bundle)?;
    write_atomically_json(paths.run_index_path.as_path(), &run_index)?;
    write_atomically_json(paths.finalized_snapshot_path.as_path(), &bundle)?;
    Ok(ParameterGolfDistributedVisualizationWriteOutcome {
        bundle,
        run_index,
        paths,
    })
}

fn read_finalizer_report(
    report_path: &Path,
) -> Result<ParameterGolfRunPod8xH100FinalizerReport, ParameterGolfDistributedVisualizationError> {
    let raw = fs::read_to_string(report_path).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Read {
            path: report_path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_str::<ParameterGolfRunPod8xH100FinalizerReport>(raw.as_str()).map_err(
        |error| ParameterGolfDistributedVisualizationError::Deserialize {
            path: report_path.display().to_string(),
            error,
        },
    )
}

fn read_distributed_receipt(
    receipt_path: &Path,
) -> Result<ParameterGolfDistributedThroughputReceipt, ParameterGolfDistributedVisualizationError> {
    let raw = fs::read_to_string(receipt_path).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Read {
            path: receipt_path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_str::<ParameterGolfDistributedThroughputReceipt>(raw.as_str()).map_err(
        |error| ParameterGolfDistributedVisualizationError::Deserialize {
            path: receipt_path.display().to_string(),
            error,
        },
    )
}

fn build_bundle_from_finalizer_report(
    report: &ParameterGolfRunPod8xH100FinalizerReport,
    report_path: &Path,
    repo_revision: String,
    observed_at_ms: u64,
    receipt: Option<&ParameterGolfDistributedThroughputReceipt>,
    existing_live_bundle: Option<&RemoteTrainingVisualizationBundle>,
    gpu_series: Vec<RemoteTrainingGpuSample>,
) -> Result<RemoteTrainingVisualizationBundle, ParameterGolfDistributedVisualizationError> {
    let run_root = PathBuf::from(report.run_root.as_str());
    let profile_id = report
        .profile_id
        .clone()
        .unwrap_or_else(|| String::from("runpod_8xh100_parameter_golf"));
    let lane_id = report
        .trainer_lane_id
        .clone()
        .unwrap_or_else(|| String::from("parameter_golf_distributed_8xh100"));
    let run_id = report.run_id.clone().unwrap_or_else(|| {
        run_root
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| String::from("parameter-golf-runpod-8xh100"))
    });
    let mut timeline = existing_live_bundle
        .map(|bundle| bundle.timeline.clone())
        .unwrap_or_else(|| {
            vec![RemoteTrainingTimelineEntry {
                observed_at_ms: observed_at_ms.saturating_sub(1_000),
                phase: String::from("training"),
                subphase: Some(String::from("distributed_runpod_8xh100")),
                detail: if receipt.is_some() {
                    String::from(
                        "The RunPod 8xH100 lane retained its distributed receipt posture and exported-folder evidence.",
                    )
                } else {
                    String::from(
                        "The RunPod 8xH100 lane retained exported-folder evidence without a coordinator-owned distributed receipt stream.",
                    )
                },
            }]
        });
    timeline.push(RemoteTrainingTimelineEntry {
        observed_at_ms,
        phase: String::from("complete"),
        subphase: Some(String::from("finalizer_sealed")),
        detail: String::from(
            "The RunPod 8xH100 finalizer sealed the provider-neutral visualization bundle.",
        ),
    });
    let mut event_series = existing_live_bundle
        .map(|bundle| bundle.event_series.clone())
        .unwrap_or_default();
    let mut runtime_series = existing_live_bundle
        .map(|bundle| bundle.runtime_series.clone())
        .unwrap_or_default();
    let mut distributed_series = existing_live_bundle
        .map(|bundle| bundle.distributed_series.clone())
        .unwrap_or_default();
    let mut heartbeat_series = existing_live_bundle
        .map(|bundle| bundle.heartbeat_series.clone())
        .unwrap_or_default();
    let loss_series = existing_live_bundle
        .map(|bundle| bundle.loss_series.clone())
        .unwrap_or_default();
    let math_series = existing_live_bundle
        .map(|bundle| bundle.math_series.clone())
        .unwrap_or_default();
    let mut combined_gpu_series = existing_live_bundle
        .map(|bundle| bundle.gpu_series.clone())
        .unwrap_or_default();
    combined_gpu_series.extend(gpu_series);
    if let Some(timing) = receipt.and_then(|receipt| receipt.timing.as_ref()) {
        runtime_series.push(RemoteTrainingRuntimeSample {
            observed_at_ms,
            data_wait_ms: None,
            forward_ms: None,
            backward_ms: None,
            optimizer_ms: Some(timing.mean_step_duration_ms),
            checkpoint_ms: Some(timing.export_observed_ms),
            evaluation_ms: Some(timing.validation_observed_ms),
            tokens_per_second: Some(timing.train_tokens_per_second),
            samples_per_second_milli: None,
        });
        distributed_series.push(RemoteTrainingDistributedSample {
            observed_at_ms,
            participating_rank_count: report.world_size.min(u64::from(u16::MAX)) as u16,
            rank_skew_ms: None,
            slowest_rank_ms: Some(timing.tail_step_duration_ms),
            collective_ms: None,
            stalled_rank_count: 0,
        });
    }
    let result_classification = match receipt.map(|receipt| receipt.disposition) {
        Some(ParameterGolfDistributedLaneDisposition::Measured) => {
            RemoteTrainingResultClassification::CompletedSuccess
        }
        Some(ParameterGolfDistributedLaneDisposition::Refused) => {
            RemoteTrainingResultClassification::Refused
        }
        None => match existing_live_bundle.map(|bundle| bundle.result_classification) {
            Some(RemoteTrainingResultClassification::CompletedSuccess) => {
                RemoteTrainingResultClassification::CompletedSuccess
            }
            Some(RemoteTrainingResultClassification::Refused) => {
                RemoteTrainingResultClassification::Refused
            }
            Some(RemoteTrainingResultClassification::RehearsalOnly) => {
                RemoteTrainingResultClassification::RehearsalOnly
            }
            _ => RemoteTrainingResultClassification::CompletedFailure,
        },
    };
    let unavailable_reason = if receipt.is_some() {
        String::from(
            "the RunPod 8xH100 lane retained distributed topology, timing, and provenance receipts, but it did not retain a coordinator-owned loss curve or live rank-skew stream",
        )
    } else {
        String::from(
            "the RunPod 8xH100 lane retained exported-folder, topology, and provenance evidence only; no distributed receipt or live loss curve was retained",
        )
    };
    if loss_series.is_empty() {
        event_series.push(RemoteTrainingEventSample {
            observed_at_ms,
            severity: RemoteTrainingEventSeverity::Warning,
            event_kind: String::from("series_unavailable"),
            detail: unavailable_reason.clone(),
        });
    }
    event_series.push(RemoteTrainingEventSample {
        observed_at_ms,
        severity: RemoteTrainingEventSeverity::Info,
        event_kind: String::from("finalizer_report_sealed"),
        detail: String::from(
            "The RunPod 8xH100 finalizer sealed the provider-neutral bundle and run index.",
        ),
    });
    if let Some(refusal) = receipt.and_then(|receipt| receipt.refusal.as_ref()) {
        event_series.push(RemoteTrainingEventSample {
            observed_at_ms,
            severity: RemoteTrainingEventSeverity::Error,
            event_kind: String::from("distributed_lane_refused"),
            detail: refusal.reason.clone(),
        });
    }
    heartbeat_series.push(RemoteTrainingHeartbeatSample {
        observed_at_ms,
        phase: String::from("complete"),
        subphase: Some(String::from("finalizer_sealed")),
        step_in_progress: None,
        microbatch_in_progress: None,
        active_subsystems: vec![
            String::from("finalizer"),
            String::from("exported_submission"),
            String::from("topology_capture"),
        ],
        stale_after_ms: PARAMETER_GOLF_DISTRIBUTED_LIVE_STALE_AFTER_MS,
    });
    let series_status = if loss_series.is_empty() {
        RemoteTrainingSeriesStatus::Unavailable
    } else {
        RemoteTrainingSeriesStatus::Available
    };
    let series_unavailable_reason =
        (series_status != RemoteTrainingSeriesStatus::Available).then_some(unavailable_reason);
    let latest_loss = loss_series.last().cloned();
    let latest_runtime = runtime_series.last().cloned();
    let total_steps_completed = existing_live_bundle
        .map(|bundle| bundle.summary.total_steps_completed)
        .unwrap_or(0)
        .max(
            receipt
                .and_then(|distributed| distributed.timing.as_ref())
                .map_or(0, |timing| timing.step_count),
        );
    let latest_global_step = latest_loss
        .as_ref()
        .and_then(|sample| sample.global_step)
        .or((total_steps_completed > 0).then_some(total_steps_completed));
    let latest_tokens_per_second = latest_runtime
        .as_ref()
        .and_then(|sample| sample.tokens_per_second)
        .or_else(|| {
            receipt
                .and_then(|distributed| distributed.timing.as_ref())
                .map(|timing| timing.train_tokens_per_second)
        });
    let source_artifacts = merge_source_artifacts(
        existing_live_bundle
            .map(|bundle| bundle.source_artifacts.clone())
            .unwrap_or_default(),
        build_source_artifacts(report, report_path, receipt),
    );
    Ok(record_remote_training_visualization_bundle(
        RemoteTrainingVisualizationBundle {
            schema_version: String::new(),
            bundle_id: format!("{run_id}-distributed-8xh100-remote-training-v1"),
            provider: RemoteTrainingProvider::RunPod,
            profile_id,
            lane_id,
            run_id,
            repo_revision,
            result_classification,
            refresh_contract: RemoteTrainingRefreshContract {
                target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
                emission_mode: existing_live_bundle
                    .map(|bundle| bundle.refresh_contract.emission_mode)
                    .unwrap_or(RemoteTrainingEmissionMode::PostRunOnly),
                last_heartbeat_at_ms: Some(observed_at_ms),
                heartbeat_seq: heartbeat_series.len() as u64,
            },
            series_status,
            series_unavailable_reason,
            timeline,
            summary: RemoteTrainingVisualizationSummary {
                total_steps_completed,
                latest_global_step,
                latest_train_loss: latest_loss.as_ref().and_then(|sample| sample.train_loss),
                latest_ema_loss: latest_loss.as_ref().and_then(|sample| sample.ema_loss),
                latest_validation_loss: loss_series
                    .iter()
                    .rev()
                    .find_map(|sample| sample.validation_loss),
                latest_tokens_per_second,
                latest_samples_per_second_milli: None,
                accumulated_cost_microusd: None,
                latest_checkpoint_ref: existing_live_bundle
                    .and_then(|bundle| bundle.summary.latest_checkpoint_ref.clone()),
                detail: if !loss_series.is_empty() {
                    String::from(
                        "The RunPod 8xH100 lane sealed its always-live distributed bundle with retained heartbeat, step, validation, GPU, and provenance series.",
                    )
                } else if receipt.is_some() {
                    String::from(
                        "The RunPod 8xH100 lane retains distributed topology, timing, GPU, and provenance truth, but it still does not retain a live loss curve.",
                    )
                } else {
                    String::from(
                        "The RunPod 8xH100 lane retains GPU inventory, exported-folder, and provenance truth while remaining explicit that no live distributed trainer telemetry was retained.",
                    )
                },
            },
            heartbeat_series,
            loss_series,
            math_series,
            runtime_series,
            gpu_series: combined_gpu_series,
            distributed_series,
            event_series,
            source_artifacts,
            bundle_digest: String::new(),
        },
    )?)
}

fn build_run_index(
    report: &ParameterGolfRunPod8xH100FinalizerReport,
    bundle: &RemoteTrainingVisualizationBundle,
) -> Result<RemoteTrainingRunIndex, ParameterGolfDistributedVisualizationError> {
    Ok(build_remote_training_run_index(RemoteTrainingRunIndex {
        schema_version: String::new(),
        index_id: format!("{}-remote-training-index-v1", bundle.run_id),
        generated_at_ms: unix_time_ms(),
        entries: vec![RemoteTrainingRunIndexEntry {
            provider: bundle.provider,
            profile_id: bundle.profile_id.clone(),
            lane_id: bundle.lane_id.clone(),
            run_id: bundle.run_id.clone(),
            repo_revision: bundle.repo_revision.clone(),
            result_classification: bundle.result_classification,
            series_status: bundle.series_status,
            series_unavailable_reason: bundle.series_unavailable_reason.clone(),
            last_heartbeat_at_ms: bundle.refresh_contract.last_heartbeat_at_ms,
            bundle_artifact_uri: Some(format!(
                "{}/{}",
                PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_DIR_NAME,
                PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_BUNDLE_NAME
            )),
            bundle_digest: Some(bundle.bundle_digest.clone()),
            summary_label: format!(
                "RunPod {} distributed 8xH100 live bundle",
                report
                    .profile_id
                    .as_deref()
                    .unwrap_or("runpod_8xh100_parameter_golf")
            ),
            detail: bundle.summary.detail.clone(),
        }],
        detail: String::from(
            "This run index enumerates the RunPod distributed 8xH100 Parameter Golf visualization bundle for normalized app discovery.",
        ),
        index_digest: String::new(),
    })?)
}

fn spawn_background_writer(
    metadata: ParameterGolfDistributedVisualizationMetadata,
    paths: ParameterGolfDistributedVisualizationPaths,
    shared: Arc<Mutex<ParameterGolfDistributedVisualizationShared>>,
    stop_requested: Arc<AtomicBool>,
) -> JoinHandle<()> {
    thread::spawn(move || {
        while !stop_requested.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_millis(
                REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
            ));
            if stop_requested.load(Ordering::SeqCst) {
                break;
            }
            let mut shared = shared.lock().expect("writer shared state should lock");
            if let Err(error) = flush_live_locked(&metadata, &paths, &mut shared, false) {
                shared.background_error = Some(error.to_string());
                stop_requested.store(true, Ordering::SeqCst);
                break;
            }
        }
    })
}

fn flush_live_locked(
    metadata: &ParameterGolfDistributedVisualizationMetadata,
    paths: &ParameterGolfDistributedVisualizationPaths,
    shared: &mut ParameterGolfDistributedVisualizationShared,
    force: bool,
) -> Result<(), ParameterGolfDistributedVisualizationError> {
    let observed_at_ms = unix_time_ms();
    if !force
        && shared.last_flush_at_ms.is_some_and(|last_flush_at_ms| {
            observed_at_ms.saturating_sub(last_flush_at_ms)
                < REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS
        })
    {
        return Ok(());
    }
    shared.state.heartbeat_seq = shared.state.heartbeat_seq.saturating_add(1);
    shared
        .state
        .heartbeat_series
        .push(RemoteTrainingHeartbeatSample {
            observed_at_ms,
            phase: shared.state.phase.clone(),
            subphase: shared.state.subphase.clone(),
            step_in_progress: shared.state.step_in_progress,
            microbatch_in_progress: None,
            active_subsystems: dedup_strings(shared.state.active_subsystems.clone()),
            stale_after_ms: PARAMETER_GOLF_DISTRIBUTED_LIVE_STALE_AFTER_MS,
        });
    shared
        .state
        .gpu_series
        .extend(collect_local_gpu_samples(observed_at_ms));
    let bundle = build_bundle_from_live_state(metadata, &shared.state)?;
    let run_index = build_live_run_index(metadata, &bundle)?;
    write_atomically_json(paths.bundle_path.as_path(), &bundle)?;
    write_atomically_json(paths.run_index_path.as_path(), &run_index)?;
    let snapshot_path = paths
        .snapshot_dir
        .join(format!("heartbeat_{:08}.json", shared.state.heartbeat_seq));
    write_atomically_json(snapshot_path.as_path(), &bundle)?;
    shared.last_flush_at_ms = Some(observed_at_ms);
    Ok(())
}

fn build_bundle_from_live_state(
    metadata: &ParameterGolfDistributedVisualizationMetadata,
    state: &ParameterGolfDistributedVisualizationState,
) -> Result<RemoteTrainingVisualizationBundle, ParameterGolfDistributedVisualizationError> {
    let (series_status, series_unavailable_reason) = if state.loss_series.is_empty() {
        let reason = if state.result_classification == RemoteTrainingResultClassification::Active {
            String::from(
                "the RunPod 8xH100 lane has not retained its first train-step or validation sample yet",
            )
        } else {
            String::from(
                "the RunPod 8xH100 lane finished without any retained train-step or validation samples",
            )
        };
        let status = if state.result_classification == RemoteTrainingResultClassification::Active {
            RemoteTrainingSeriesStatus::Partial
        } else {
            RemoteTrainingSeriesStatus::Unavailable
        };
        (status, Some(reason))
    } else {
        (RemoteTrainingSeriesStatus::Available, None)
    };
    let latest_loss = state.loss_series.last().cloned();
    let latest_runtime = state.runtime_series.last().cloned();
    Ok(record_remote_training_visualization_bundle(
        RemoteTrainingVisualizationBundle {
            schema_version: String::new(),
            bundle_id: format!("{}-distributed-8xh100-live-v1", state.run_id),
            provider: metadata.provider,
            profile_id: metadata.profile_id.clone(),
            lane_id: metadata.lane_id.clone(),
            run_id: state.run_id.clone(),
            repo_revision: metadata.repo_revision.clone(),
            result_classification: state.result_classification,
            refresh_contract: RemoteTrainingRefreshContract {
                target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
                emission_mode: RemoteTrainingEmissionMode::AppendOnlySnapshots,
                last_heartbeat_at_ms: state
                    .heartbeat_series
                    .last()
                    .map(|sample| sample.observed_at_ms),
                heartbeat_seq: state.heartbeat_seq,
            },
            series_status,
            series_unavailable_reason,
            timeline: state.timeline.clone(),
            summary: RemoteTrainingVisualizationSummary {
                total_steps_completed: state.total_steps_completed,
                latest_global_step: latest_loss.as_ref().and_then(|sample| sample.global_step),
                latest_train_loss: latest_loss.as_ref().and_then(|sample| sample.train_loss),
                latest_ema_loss: latest_loss.as_ref().and_then(|sample| sample.ema_loss),
                latest_validation_loss: state
                    .loss_series
                    .iter()
                    .rev()
                    .find_map(|sample| sample.validation_loss),
                latest_tokens_per_second: latest_runtime
                    .as_ref()
                    .and_then(|sample| sample.tokens_per_second),
                latest_samples_per_second_milli: None,
                accumulated_cost_microusd: None,
                latest_checkpoint_ref: state.latest_checkpoint_ref.clone(),
                detail: state.summary_detail.clone(),
            },
            heartbeat_series: state.heartbeat_series.clone(),
            loss_series: state.loss_series.clone(),
            math_series: state.math_series.clone(),
            runtime_series: state.runtime_series.clone(),
            gpu_series: state.gpu_series.clone(),
            distributed_series: state.distributed_series.clone(),
            event_series: state.event_series.clone(),
            source_artifacts: build_live_source_artifacts(state),
            bundle_digest: String::new(),
        },
    )?)
}

fn build_live_run_index(
    metadata: &ParameterGolfDistributedVisualizationMetadata,
    bundle: &RemoteTrainingVisualizationBundle,
) -> Result<RemoteTrainingRunIndex, ParameterGolfDistributedVisualizationError> {
    Ok(build_remote_training_run_index(RemoteTrainingRunIndex {
        schema_version: String::new(),
        index_id: format!("{}-distributed-live-index-v1", bundle.run_id),
        generated_at_ms: unix_time_ms(),
        entries: vec![RemoteTrainingRunIndexEntry {
            provider: bundle.provider,
            profile_id: bundle.profile_id.clone(),
            lane_id: bundle.lane_id.clone(),
            run_id: bundle.run_id.clone(),
            repo_revision: bundle.repo_revision.clone(),
            result_classification: bundle.result_classification,
            series_status: bundle.series_status,
            series_unavailable_reason: bundle.series_unavailable_reason.clone(),
            last_heartbeat_at_ms: bundle.refresh_contract.last_heartbeat_at_ms,
            bundle_artifact_uri: Some(metadata.bundle_relative_path()),
            bundle_digest: Some(bundle.bundle_digest.clone()),
            summary_label: String::from("RunPod 8xH100 PGOLF distributed live sample"),
            detail: bundle.summary.detail.clone(),
        }],
        detail: String::from(
            "This run index enumerates the RunPod distributed 8xH100 Parameter Golf live bundle for normalized app discovery.",
        ),
        index_digest: String::new(),
    })?)
}

fn build_live_source_artifacts(
    state: &ParameterGolfDistributedVisualizationState,
) -> Vec<RemoteTrainingSourceArtifact> {
    state.source_artifacts.values().cloned().collect()
}

fn merge_source_artifacts(
    existing: Vec<RemoteTrainingSourceArtifact>,
    additional: Vec<RemoteTrainingSourceArtifact>,
) -> Vec<RemoteTrainingSourceArtifact> {
    let mut merged = BTreeMap::new();
    for artifact in existing.into_iter().chain(additional) {
        merged.insert(
            format!("{}|{}", artifact.artifact_role, artifact.artifact_uri),
            artifact,
        );
    }
    merged.into_values().collect()
}

fn build_source_artifacts(
    report: &ParameterGolfRunPod8xH100FinalizerReport,
    report_path: &Path,
    receipt: Option<&ParameterGolfDistributedThroughputReceipt>,
) -> Vec<RemoteTrainingSourceArtifact> {
    let run_root = PathBuf::from(report.run_root.as_str());
    let mut artifacts = vec![
        RemoteTrainingSourceArtifact {
            artifact_role: String::from("live_bundle"),
            artifact_uri: format!(
                "{}/{}",
                PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_DIR_NAME,
                PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_BUNDLE_NAME
            ),
            artifact_digest: None,
            source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
            authoritative: true,
            source_receipt_ids: vec![String::from(
                "parameter_golf_distributed_8xh100.remote_training_bundle.v1",
            )],
            detail: String::from(
                "The provider-neutral local mirror is the app-facing source for the RunPod distributed 8xH100 lane.",
            ),
        },
        RemoteTrainingSourceArtifact {
            artifact_role: String::from("finalizer_report"),
            artifact_uri: relative_path_string(run_root.as_path(), report_path)
                .unwrap_or_else(|| report_path.display().to_string()),
            artifact_digest: None,
            source_kind: RemoteTrainingArtifactSourceKind::FinalizerOwned,
            authoritative: true,
            source_receipt_ids: vec![String::from("parameter_golf.runpod_8xh100_finalizer.v1")],
            detail: String::from(
                "The finalizer report remains authoritative for exported-folder, topology, and inventory provenance.",
            ),
        },
        RemoteTrainingSourceArtifact {
            artifact_role: String::from("gpu_inventory"),
            artifact_uri: relative_path_string(
                run_root.as_path(),
                Path::new(report.accelerator_evidence.inventory_path.as_str()),
            )
            .unwrap_or_else(|| report.accelerator_evidence.inventory_path.clone()),
            artifact_digest: None,
            source_kind: RemoteTrainingArtifactSourceKind::ProviderGenerated,
            authoritative: true,
            source_receipt_ids: Vec::new(),
            detail: String::from(
                "The `nvidia-smi` inventory snapshot remains authoritative for the retained per-device GPU state.",
            ),
        },
        RemoteTrainingSourceArtifact {
            artifact_role: String::from("gpu_topology"),
            artifact_uri: relative_path_string(
                run_root.as_path(),
                Path::new(report.accelerator_evidence.topology_path.as_str()),
            )
            .unwrap_or_else(|| report.accelerator_evidence.topology_path.clone()),
            artifact_digest: None,
            source_kind: RemoteTrainingArtifactSourceKind::ProviderGenerated,
            authoritative: true,
            source_receipt_ids: Vec::new(),
            detail: String::from(
                "The `nvidia-smi topo -m` capture remains authoritative for the retained fabric topology snapshot.",
            ),
        },
    ];
    artifacts.push(RemoteTrainingSourceArtifact {
        artifact_role: String::from("submission_manifest"),
        artifact_uri: relative_path_string(
            run_root.as_path(),
            Path::new(report.exported_folder_evidence.submission_manifest_path.as_str()),
        )
        .unwrap_or_else(|| report.exported_folder_evidence.submission_manifest_path.clone()),
        artifact_digest: report
            .exported_folder_evidence
            .submission_manifest_sha256
            .clone(),
        source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
        authoritative: true,
        source_receipt_ids: vec![String::from("parameter_golf.submission_manifest.v1")],
        detail: String::from(
            "The exported submission manifest remains authoritative for the retained folder contract.",
        ),
    });
    if let Some(path) = &report.exported_folder_evidence.submission_run_evidence_path {
        artifacts.push(RemoteTrainingSourceArtifact {
            artifact_role: String::from("submission_run_evidence"),
            artifact_uri: relative_path_string(run_root.as_path(), Path::new(path.as_str()))
                .unwrap_or_else(|| path.clone()),
            artifact_digest: report
                .exported_folder_evidence
                .submission_run_evidence_sha256
                .clone(),
            source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
            authoritative: true,
            source_receipt_ids: vec![String::from("parameter_golf.submission_run_evidence.v1")],
            detail: String::from(
                "The submission run evidence report remains authoritative for the exported-folder execution posture and digests.",
            ),
        });
    }
    if let Some(receipt) = receipt {
        artifacts.push(RemoteTrainingSourceArtifact {
            artifact_role: String::from("distributed_receipt"),
            artifact_uri: report
                .exported_folder_evidence
                .distributed_receipt_path
                .clone()
                .and_then(|path| relative_path_string(run_root.as_path(), Path::new(path.as_str())))
                .unwrap_or_else(|| String::from("parameter_golf_distributed_8xh100_receipt.json")),
            artifact_digest: Some(receipt.receipt_digest.clone()),
            source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
            authoritative: true,
            source_receipt_ids: vec![String::from(
                "parameter_golf_distributed_throughput_receipt",
            )],
            detail: String::from(
                "The distributed throughput receipt remains authoritative for retained topology, communication, timing, and memory posture.",
            ),
        });
    }
    artifacts
}

fn read_visualization_bundle(
    bundle_path: &Path,
) -> Result<Option<RemoteTrainingVisualizationBundle>, ParameterGolfDistributedVisualizationError> {
    if !bundle_path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(bundle_path).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Read {
            path: bundle_path.display().to_string(),
            error,
        }
    })?;
    let bundle = serde_json::from_str::<RemoteTrainingVisualizationBundle>(raw.as_str()).map_err(
        |error| ParameterGolfDistributedVisualizationError::Deserialize {
            path: bundle_path.display().to_string(),
            error,
        },
    )?;
    bundle.validate()?;
    Ok(Some(bundle))
}

fn read_run_index(
    run_index_path: &Path,
) -> Result<Option<RemoteTrainingRunIndex>, ParameterGolfDistributedVisualizationError> {
    if !run_index_path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(run_index_path).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Read {
            path: run_index_path.display().to_string(),
            error,
        }
    })?;
    let run_index =
        serde_json::from_str::<RemoteTrainingRunIndex>(raw.as_str()).map_err(|error| {
            ParameterGolfDistributedVisualizationError::Deserialize {
                path: run_index_path.display().to_string(),
                error,
            }
        })?;
    run_index.validate()?;
    Ok(Some(run_index))
}

fn read_gpu_inventory(
    inventory_path: &Path,
    observed_at_ms: u64,
) -> Result<Vec<RemoteTrainingGpuSample>, ParameterGolfDistributedVisualizationError> {
    if !inventory_path.is_file() {
        return Ok(Vec::new());
    }
    let raw = fs::read_to_string(inventory_path).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Read {
            path: inventory_path.display().to_string(),
            error,
        }
    })?;
    Ok(raw
        .lines()
        .enumerate()
        .filter_map(|(index, line)| parse_gpu_inventory_row(index, line, observed_at_ms))
        .collect())
}

fn parse_gpu_inventory_row(
    _index: usize,
    line: &str,
    observed_at_ms: u64,
) -> Option<RemoteTrainingGpuSample> {
    let parts = line.split(',').map(|part| part.trim()).collect::<Vec<_>>();
    if parts.len() < 5 {
        return None;
    }
    let device_id = format!("cuda:{}", parse_u64(parts.first()?)?);
    let device_label = parts.get(1)?.to_string();
    let memory_total_mib = parse_u64(parts.get(2)?)?;
    let memory_used_mib = parse_u64(parts.get(3)?)?;
    let utilization_percent = parse_u64(parts.get(4)?)?;
    Some(RemoteTrainingGpuSample {
        observed_at_ms,
        device_id,
        device_label,
        utilization_bps: (utilization_percent.min(100) as u32).saturating_mul(100),
        memory_used_bytes: memory_used_mib.saturating_mul(1024 * 1024),
        memory_total_bytes: memory_total_mib.saturating_mul(1024 * 1024),
        temperature_celsius: None,
        power_watts: None,
    })
}

fn parse_u64(raw: &str) -> Option<u64> {
    raw.chars()
        .filter(|character| character.is_ascii_digit())
        .collect::<String>()
        .parse::<u64>()
        .ok()
}

fn collect_local_gpu_samples(observed_at_ms: u64) -> Vec<RemoteTrainingGpuSample> {
    let Ok(output) = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw",
            "--format=csv,noheader,nounits",
        ])
        .output()
    else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let columns = line
                .split(',')
                .map(|value| value.trim())
                .collect::<Vec<_>>();
            if columns.len() < 5 {
                return None;
            }
            let memory_used_mib = columns.get(3)?.parse::<u64>().ok()?;
            let memory_total_mib = columns.get(4)?.parse::<u64>().ok()?;
            Some(RemoteTrainingGpuSample {
                observed_at_ms,
                device_id: format!("cuda:{}", columns.first()?.parse::<u32>().ok()?),
                device_label: columns.get(1)?.to_string(),
                utilization_bps: columns.get(2)?.parse::<u32>().ok()?.saturating_mul(100),
                memory_used_bytes: memory_used_mib.saturating_mul(1024 * 1024),
                memory_total_bytes: memory_total_mib.saturating_mul(1024 * 1024),
                temperature_celsius: columns.get(5).and_then(|value| value.parse::<u16>().ok()),
                power_watts: columns.get(6).and_then(|value| {
                    let trimmed = value.trim();
                    if trimmed == "[Not Supported]" {
                        None
                    } else {
                        trimmed
                            .split('.')
                            .next()
                            .and_then(|whole| whole.parse::<u16>().ok())
                    }
                }),
            })
        })
        .collect()
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        if value.trim().is_empty() || !seen.insert(value.clone()) {
            continue;
        }
        output.push(value);
    }
    output
}

fn mean_rank_timing_ms(
    receipt: &ParameterGolfDistributed8xH100TrainStepReceipt,
    project: impl Fn(&crate::ParameterGolfDistributed8xH100TrainStepRankReceipt) -> u64,
) -> Option<u64> {
    let mut sum = 0_u128;
    let mut count = 0_u128;
    for launch in &receipt.rank_launches {
        let Some(child) = launch.receipt.as_ref() else {
            return None;
        };
        sum = sum.saturating_add(u128::from(project(child)));
        count = count.saturating_add(1);
    }
    (count > 0).then_some((sum / count) as u64)
}

fn tokens_per_second(tokens: u64, observed_ms: u64) -> u64 {
    if observed_ms == 0 {
        return 0;
    }
    ((u128::from(tokens)).saturating_mul(1000) / u128::from(observed_ms)) as u64
}

fn visualization_paths(run_root: &Path) -> ParameterGolfDistributedVisualizationPaths {
    let visualization_dir = run_root.join(PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_DIR_NAME);
    let snapshot_dir =
        visualization_dir.join(PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_SNAPSHOT_DIR_NAME);
    ParameterGolfDistributedVisualizationPaths {
        bundle_path: visualization_dir.join(PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_BUNDLE_NAME),
        run_index_path: visualization_dir
            .join(PARAMETER_GOLF_DISTRIBUTED_VISUALIZATION_RUN_INDEX_NAME),
        snapshot_dir: snapshot_dir.clone(),
        finalized_snapshot_path: snapshot_dir.join("finalized_bundle.json"),
    }
}

fn write_atomically_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), ParameterGolfDistributedVisualizationError> {
    let parent = path
        .parent()
        .expect("json output path should have a parent");
    fs::create_dir_all(parent).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Write {
            path: parent.display().to_string(),
            error,
        }
    })?;
    let tmp_path = path.with_extension("tmp");
    let encoded = serde_json::to_string_pretty(value).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Serialize {
            path: path.display().to_string(),
            error,
        }
    })?;
    fs::write(tmp_path.as_path(), format!("{encoded}\n")).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Write {
            path: tmp_path.display().to_string(),
            error,
        }
    })?;
    fs::rename(tmp_path.as_path(), path).map_err(|error| {
        ParameterGolfDistributedVisualizationError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn relative_path_string(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use tempfile::tempdir;

    use super::*;
    use psionic_eval::{
        ParameterGolfDistributedChallengeThresholds, ParameterGolfDistributedCommunicationReceipt,
        ParameterGolfDistributedCommunicationStageReceipt, ParameterGolfDistributedTimingReceipt,
        ParameterGolfDistributedTopologyReceipt,
        ParameterGolfDistributedValidationAggregationReceipt,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF,
        PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY,
    };
    use psionic_runtime::{
        BackendSelection, ClusterCommunicationClass, ClusterTransportClass, TrainingCollectiveKind,
        TrainingCollectiveQuantization, TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind,
    };

    use crate::{
        ParameterGolfDistributed8xH100TrainStepRankLaunch,
        ParameterGolfDistributed8xH100TrainStepRankReceipt,
        ParameterGolfDistributedStepObservation, ParameterGolfSingleH100PhaseTimings,
    };

    fn sample_distributed_receipt() -> ParameterGolfDistributedThroughputReceipt {
        ParameterGolfDistributedThroughputReceipt {
            benchmark_ref: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_BENCHMARK_REF),
            run_id: String::from("parameter-golf-runpod-test"),
            model_descriptor_digest: String::from("model-digest"),
            optimizer_plan_digest: String::from("optimizer-digest"),
            thresholds: ParameterGolfDistributedChallengeThresholds::challenge_8xh100(),
            topology: ParameterGolfDistributedTopologyReceipt {
                backend_selection: BackendSelection::direct(
                    "cuda",
                    None,
                    vec![String::from("parameter_golf_distributed_train")],
                ),
                topology_digest: String::from("topology-digest"),
                selected_device_names: vec![String::from("NVIDIA H100 80GB HBM3"); 8],
                all_devices_match_required_model: true,
            },
            communication: ParameterGolfDistributedCommunicationReceipt {
                communication_class: ClusterCommunicationClass::TensorCollectiveMesh,
                transport: ClusterTransportClass::Loopback,
                mesh_id: String::from("mesh.parameter_golf.8xh100"),
                axes: vec![TrainingDeviceMeshAxis::new(
                    "dp",
                    TrainingDeviceMeshAxisKind::DataParallel,
                    8,
                )
                .with_collective_group_size(8)],
                stages: vec![ParameterGolfDistributedCommunicationStageReceipt {
                    stage_id: String::from("ddp_gradient_all_reduce"),
                    collective_kind: TrainingCollectiveKind::AllReduce,
                    quantization: TrainingCollectiveQuantization::None,
                    payload_bytes: 1024,
                    estimated_wire_bytes: 2048,
                    worker_count: 8,
                    detail: String::from("DDP gradient synchronization"),
                }],
            },
            training_capability_report_digest: String::from("coverage-digest"),
            challenge_kernel_blockers: vec![String::from("collective.rank_skew_missing")],
            disposition: ParameterGolfDistributedLaneDisposition::Measured,
            timing: Some(ParameterGolfDistributedTimingReceipt {
                measurement_posture: String::from("observed_step_wallclock"),
                step_count: 4,
                total_train_tokens: 1_048_576,
                training_step_observed_ms: 400,
                validation_observed_ms: 20,
                export_observed_ms: 10,
                total_observed_ms: 430,
                mean_step_duration_ms: 100,
                tail_step_duration_ms: 112,
                train_tokens_per_second: 2_621_440,
                wallclock_cap_ms: 600_000,
                within_wallclock_cap: true,
            }),
            validation_aggregation: Some(ParameterGolfDistributedValidationAggregationReceipt {
                measurement_posture: String::from("distributed_validation"),
                eval_mode: String::from("non_overlapping"),
                world_size: 8,
                total_sequence_count: 1024,
                total_evaluation_unit_count: 1024,
                local_batch_sequences: 128,
                aggregated_loss_sum: 3_814.4,
                aggregated_token_count: 65_536,
                aggregated_byte_count: 131_072,
                mean_loss: 3.82,
                bits_per_byte: 1.91,
                observed_ms: 20,
                shards: Vec::new(),
            }),
            memory: None,
            refusal: None,
            boundary_notes: vec![String::from("boundary")],
            claim_boundary: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY),
            receipt_digest: String::new(),
        }
        .with_stable_digest()
    }

    fn sample_train_step_receipt(
        run_root: &Path,
    ) -> ParameterGolfDistributed8xH100TrainStepReceipt {
        let bringup_report_path = run_root.join("parameter_golf_distributed_8xh100_bringup.json");
        let runtime_manifest_path = run_root.join("runtime_manifest.json");
        let runtime_bootstrap_receipt_path =
            run_root.join("parameter_golf_distributed_8xh100_runtime_bootstrap.json");
        let train_step_receipt_path =
            run_root.join("parameter_golf_distributed_8xh100_train_step.json");
        let distributed_receipt_path =
            run_root.join("parameter_golf_distributed_8xh100_receipt.json");
        let measurements_path =
            run_root.join("parameter_golf_distributed_8xh100_measurements.json");
        let model_artifact_path = run_root.join("current_model.runtime_surface.safetensors");
        let model_int8_artifact_path = run_root.join("current_model.int8.zlib");
        let gradient_path = run_root.join("aggregated_gradients.safetensors");
        fs::write(
            bringup_report_path.as_path(),
            "{\n  \"schema_version\": 1\n}\n",
        )
        .expect("write bringup report");
        fs::write(
            runtime_manifest_path.as_path(),
            "{\n  \"schema_version\": 1\n}\n",
        )
        .expect("write runtime manifest");
        fs::write(
            runtime_bootstrap_receipt_path.as_path(),
            "{\n  \"schema_version\": 1\n}\n",
        )
        .expect("write runtime bootstrap receipt");
        fs::write(
            measurements_path.as_path(),
            "{\n  \"schema_version\": 1\n}\n",
        )
        .expect("write measurements receipt");
        fs::write(model_artifact_path.as_path(), b"model").expect("write model artifact");
        fs::write(model_int8_artifact_path.as_path(), b"model-int8")
            .expect("write model int8 artifact");
        fs::write(gradient_path.as_path(), b"grads").expect("write gradient artifact");

        let rank_launches = (0..8)
            .map(|rank| {
                let receipt_path = run_root.join(format!("rank_{rank}_receipt.json"));
                let log_path = run_root.join(format!("rank_{rank}.log"));
                let window_path = run_root.join(format!("rank_{rank}_window.json"));
                let gradient_artifact_path =
                    run_root.join(format!("rank_{rank}_gradient.safetensors"));
                fs::write(log_path.as_path(), format!("rank {rank}\n")).expect("write rank log");
                fs::write(window_path.as_path(), "{\n  \"window\": 1\n}\n")
                    .expect("write rank window");
                fs::write(gradient_artifact_path.as_path(), b"rank-gradient")
                    .expect("write rank gradient");
                let mut receipt = ParameterGolfDistributed8xH100TrainStepRankReceipt {
                    schema_version: 1,
                    run_id: String::from("parameter-golf-runpod-test"),
                    rank,
                    local_rank: rank,
                    world_size: 8,
                    cuda_visible_devices: rank.to_string(),
                    selected_device_label: String::from("NVIDIA H100 80GB HBM3"),
                    log_path: log_path.display().to_string(),
                    window_path: window_path.display().to_string(),
                    window_id: format!("window-{rank}"),
                    gradient_artifact_path: gradient_artifact_path.display().to_string(),
                    gradient_artifact_sha256: format!("gradient-sha-{rank}"),
                    gradient_sync_transport: String::from("file_artifact_parent_aggregation_v1"),
                    input_model_artifact_path: Some(model_artifact_path.display().to_string()),
                    input_model_artifact_sha256: Some(String::from("model-sha")),
                    loss: 4.25 - (rank as f32 * 0.01),
                    phase_timings: ParameterGolfSingleH100PhaseTimings {
                        window_planning_ms: 2,
                        token_materialization_ms: 3,
                        embedding_gather_ms: 4,
                        forward_loss_cuda_ms: 30 + rank as u64,
                        retained_forward_readback_ms: 2,
                        backward_cuda_ms: 40 + rank as u64,
                        gradient_readback_ms: 3,
                        host_gradient_materialization_ms: 4,
                        optimizer_step_ms: 5,
                        retained_binding_tensor_count: 128,
                        retained_binding_f32_count: 4096,
                        gradient_tensor_count: 128,
                        gradient_f32_count: 4096,
                    },
                    runtime_receipt: None,
                    observed_wallclock_ms: 100 + rank as u64,
                    gradient_sync_ms: 8,
                    optimizer_step_ms: 6,
                    gradient_norm_after_clip: 1.25,
                    clip_applied: false,
                    non_finite_gradient_count: 0,
                    parallel_muon_receipt: None,
                    worker_pid: 10_000 + rank as u32,
                    claim_boundary: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY),
                    receipt_digest: String::new(),
                };
                receipt.receipt_digest = receipt.stable_digest();
                fs::write(
                    receipt_path.as_path(),
                    format!(
                        "{}\n",
                        serde_json::to_string_pretty(&receipt).expect("encode rank receipt")
                    ),
                )
                .expect("write rank receipt");
                ParameterGolfDistributed8xH100TrainStepRankLaunch {
                    rank,
                    local_rank: rank,
                    cuda_visible_devices: rank.to_string(),
                    window_path: window_path.display().to_string(),
                    gradient_artifact_path: gradient_artifact_path.display().to_string(),
                    receipt_path: receipt_path.display().to_string(),
                    log_path: log_path.display().to_string(),
                    exit_code: Some(0),
                    receipt: Some(receipt),
                }
            })
            .collect::<Vec<_>>();
        let distributed_receipt = sample_distributed_receipt();
        fs::write(
            distributed_receipt_path.as_path(),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&distributed_receipt)
                    .expect("encode distributed receipt")
            ),
        )
        .expect("write distributed receipt");
        let mut receipt = ParameterGolfDistributed8xH100TrainStepReceipt {
            schema_version: 1,
            run_id: String::from("parameter-golf-runpod-test"),
            world_size: 8,
            bringup_report_path: bringup_report_path.display().to_string(),
            bringup_report_digest: String::from("bringup-digest"),
            runtime_bootstrap_receipt_path: runtime_bootstrap_receipt_path.display().to_string(),
            runtime_bootstrap_receipt_digest: String::from("runtime-bootstrap-digest"),
            runtime_payload_path: String::from("runtime_payload.tar.zst"),
            runtime_manifest_path: runtime_manifest_path.display().to_string(),
            measurements_path: measurements_path.display().to_string(),
            distributed_receipt_path: distributed_receipt_path.display().to_string(),
            train_step_receipt_path: train_step_receipt_path.display().to_string(),
            step_scope_root_dir: run_root.join("runtime_step_scopes").display().to_string(),
            executed_step_count: 1,
            observed_training_time_ms: 400,
            step_observations: vec![ParameterGolfDistributedStepObservation::new(
                1,
                1_742_846_401_000,
                1_742_846_401_400,
                1_048_576,
            )],
            stop_reason: Some(String::from("iteration_cap_reached")),
            mean_train_loss: 4.12,
            train_tokens: 1_048_576,
            observed_step_ms: 400,
            gradient_sync_ms: 28,
            optimizer_step_ms: 12,
            gradient_norm_after_clip: 0.91,
            clip_applied: true,
            non_finite_gradient_count: 0,
            rank_launches,
            aggregated_gradient_artifact_path: gradient_path.display().to_string(),
            aggregated_gradient_artifact_sha256: String::from("aggregated-gradient-sha"),
            current_model_artifact_path: model_artifact_path.display().to_string(),
            current_model_artifact_sha256: String::from("model-sha"),
            current_model_artifact_surface: String::from("banked_full_precision_v1"),
            current_model_int8_zlib_artifact_path: model_int8_artifact_path.display().to_string(),
            current_model_int8_zlib_artifact_sha256: String::from("model-int8-sha"),
            current_model_int8_zlib_artifact_size_bytes: 9,
            validation_rank_launches: Vec::new(),
            step_observation: ParameterGolfDistributedStepObservation::new(
                1,
                1_742_846_401_000,
                1_742_846_401_400,
                1_048_576,
            ),
            validation_observed_ms: 20,
            validation_total_sequence_count: 1024,
            validation_shard_observations: Vec::new(),
            score_first_ttt_receipt: None,
            distributed_receipt,
            claim_boundary: String::from(PARAMETER_GOLF_DISTRIBUTED_8XH100_CLAIM_BOUNDARY),
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = receipt.stable_digest();
        fs::write(
            train_step_receipt_path.as_path(),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&receipt).expect("encode train-step receipt")
            ),
        )
        .expect("write train-step receipt");
        receipt
    }

    #[test]
    fn finalizer_report_writes_unavailable_bundle_without_distributed_receipt(
    ) -> Result<(), Box<dyn Error>> {
        let tempdir = tempdir()?;
        let run_root = tempdir.path().join("parameter-golf-runpod-test");
        fs::create_dir_all(run_root.as_path())?;
        let inventory_path = run_root.join("nvidia_smi_inventory.txt");
        fs::write(
            inventory_path.as_path(),
            "0, NVIDIA H100 80GB HBM3, 81559 MiB, 1024 MiB, 76 %\n1, NVIDIA H100 80GB HBM3, 81559 MiB, 2048 MiB, 71 %\n",
        )?;
        let topology_path = run_root.join("nvidia_smi_topology.txt");
        fs::write(topology_path.as_path(), "GPU0 GPU1\n")?;
        let report_path = run_root.join("finalizer_report.json");
        let report = ParameterGolfRunPod8xH100FinalizerReport {
            schema_version: String::from("parameter_golf.runpod_8xh100_finalizer.v1"),
            runner: String::from("scripts/parameter-golf-runpod-finalize-8xh100.sh"),
            created_at_utc: Some(String::from("2026-03-24T22:00:00Z")),
            run_id: Some(String::from("parameter-golf-runpod-test")),
            profile_id: Some(String::from("runpod_8xh100_parameter_golf")),
            trainer_lane_id: Some(String::from("parameter_golf_distributed_8xh100")),
            run_root: run_root.display().to_string(),
            submission_dir: run_root.join("exported_submission").display().to_string(),
            world_size: 8,
            grad_accum_steps: 1,
            accelerator_evidence: ParameterGolfRunPod8xH100AcceleratorEvidence {
                inventory_path: inventory_path.display().to_string(),
                topology_path: topology_path.display().to_string(),
                inventory_line_count: 2,
            },
            exported_folder_evidence: ParameterGolfRunPod8xH100ExportedFolderEvidence {
                entrypoint_path: String::from("train_gpt.py"),
                entrypoint_sha256: Some(String::from("entrypoint-sha")),
                submission_manifest_path: String::from("submission.json"),
                submission_manifest_sha256: Some(String::from("manifest-sha")),
                submission_run_evidence_path: None,
                submission_run_evidence_sha256: None,
                distributed_receipt_path: None,
                distributed_receipt_sha256: None,
            },
            claim_boundary: String::from("claim boundary"),
        };
        fs::write(
            report_path.as_path(),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        let outcome = write_parameter_golf_distributed_visualization_from_finalizer_report(
            report_path.as_path(),
            "workspace@test",
        )?;
        outcome.bundle.validate()?;
        outcome.run_index.validate()?;
        assert_eq!(
            outcome.bundle.refresh_contract.emission_mode,
            RemoteTrainingEmissionMode::PostRunOnly
        );
        assert_eq!(
            outcome.bundle.series_status,
            RemoteTrainingSeriesStatus::Unavailable
        );
        assert_eq!(outcome.bundle.gpu_series.len(), 2);
        Ok(())
    }

    #[test]
    fn finalizer_report_uses_distributed_receipt_when_present() -> Result<(), Box<dyn Error>> {
        let tempdir = tempdir()?;
        let run_root = tempdir.path().join("parameter-golf-runpod-test");
        fs::create_dir_all(run_root.as_path())?;
        let inventory_path = run_root.join("nvidia_smi_inventory.txt");
        fs::write(
            inventory_path.as_path(),
            "0, NVIDIA H100 80GB HBM3, 81559 MiB, 1024 MiB, 76 %\n",
        )?;
        let topology_path = run_root.join("nvidia_smi_topology.txt");
        fs::write(topology_path.as_path(), "GPU0\n")?;
        let receipt_path = run_root.join("parameter_golf_distributed_8xh100_receipt.json");
        let receipt = sample_distributed_receipt();
        fs::write(
            receipt_path.as_path(),
            format!("{}\n", serde_json::to_string_pretty(&receipt)?),
        )?;
        let report_path = run_root.join("finalizer_report.json");
        let report = ParameterGolfRunPod8xH100FinalizerReport {
            schema_version: String::from("parameter_golf.runpod_8xh100_finalizer.v1"),
            runner: String::from("scripts/parameter-golf-runpod-finalize-8xh100.sh"),
            created_at_utc: Some(String::from("2026-03-24T22:00:00Z")),
            run_id: Some(String::from("parameter-golf-runpod-test")),
            profile_id: Some(String::from("runpod_8xh100_parameter_golf")),
            trainer_lane_id: Some(String::from("parameter_golf_distributed_8xh100")),
            run_root: run_root.display().to_string(),
            submission_dir: run_root.join("exported_submission").display().to_string(),
            world_size: 8,
            grad_accum_steps: 1,
            accelerator_evidence: ParameterGolfRunPod8xH100AcceleratorEvidence {
                inventory_path: inventory_path.display().to_string(),
                topology_path: topology_path.display().to_string(),
                inventory_line_count: 1,
            },
            exported_folder_evidence: ParameterGolfRunPod8xH100ExportedFolderEvidence {
                entrypoint_path: String::from("train_gpt.py"),
                entrypoint_sha256: Some(String::from("entrypoint-sha")),
                submission_manifest_path: String::from("submission.json"),
                submission_manifest_sha256: Some(String::from("manifest-sha")),
                submission_run_evidence_path: None,
                submission_run_evidence_sha256: None,
                distributed_receipt_path: Some(receipt_path.display().to_string()),
                distributed_receipt_sha256: Some(receipt.receipt_digest.clone()),
            },
            claim_boundary: String::from("claim boundary"),
        };
        fs::write(
            report_path.as_path(),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        let outcome = write_parameter_golf_distributed_visualization_from_finalizer_report(
            report_path.as_path(),
            "workspace@test",
        )?;
        assert_eq!(
            outcome.bundle.refresh_contract.emission_mode,
            RemoteTrainingEmissionMode::PostRunOnly
        );
        assert_eq!(outcome.bundle.summary.total_steps_completed, 4);
        assert_eq!(
            outcome.bundle.summary.latest_tokens_per_second,
            Some(2_621_440)
        );
        assert_eq!(outcome.bundle.distributed_series.len(), 1);
        outcome.bundle.validate()?;
        Ok(())
    }

    #[test]
    fn live_writer_emits_append_only_bundle_every_second() -> Result<(), Box<dyn Error>> {
        let tempdir = tempdir()?;
        let run_root = tempdir.path().join("parameter-golf-live-run");
        fs::create_dir_all(run_root.as_path())?;
        let bringup_report_path = run_root.join("parameter_golf_distributed_8xh100_bringup.json");
        let runtime_manifest_path = run_root.join("runtime_manifest.json");
        fs::write(
            bringup_report_path.as_path(),
            "{\n  \"schema_version\": 1\n}\n",
        )?;
        fs::write(
            runtime_manifest_path.as_path(),
            "{\n  \"schema_version\": 1\n}\n",
        )?;
        let metadata = ParameterGolfDistributedVisualizationMetadata {
            provider: RemoteTrainingProvider::RunPod,
            profile_id: String::from("runpod_8xh100_parameter_golf"),
            lane_id: String::from("parameter_golf_distributed_8xh100"),
            repo_revision: String::from("workspace@test"),
            run_root: run_root.clone(),
            bringup_report_path,
            runtime_manifest_path,
        };
        let mut writer = ParameterGolfDistributedLiveVisualizationWriter::start(
            metadata.clone(),
            "parameter-golf-runpod-test",
            "The RunPod runtime started.",
        )?;
        let initial_bundle = read_visualization_bundle(metadata.paths().bundle_path.as_path())?
            .expect("initial bundle should exist");
        assert_eq!(
            initial_bundle.refresh_contract.emission_mode,
            RemoteTrainingEmissionMode::AppendOnlySnapshots
        );
        assert_eq!(
            initial_bundle.result_classification,
            RemoteTrainingResultClassification::Active
        );
        assert_eq!(
            initial_bundle.series_status,
            RemoteTrainingSeriesStatus::Partial
        );
        std::thread::sleep(Duration::from_millis(
            REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS + 250,
        ));
        let heartbeat_bundle = read_visualization_bundle(metadata.paths().bundle_path.as_path())?
            .expect("heartbeat bundle should exist");
        assert!(heartbeat_bundle.refresh_contract.heartbeat_seq >= 2);
        let receipt = sample_train_step_receipt(run_root.as_path());
        writer.record_phase(
            "training",
            Some(String::from("distributed_train_step")),
            "The distributed lane is executing optimizer step 1.",
            vec![String::from("train_step"), String::from("validation")],
            Some(1),
            true,
        )?;
        writer.record_train_step_receipt(&receipt)?;
        let outcome = writer.finish(
            RemoteTrainingResultClassification::CompletedSuccess,
            "The runtime completed and sealed the retained live series.",
        )?;
        outcome.bundle.validate()?;
        outcome.run_index.validate()?;
        assert_eq!(
            outcome.bundle.refresh_contract.emission_mode,
            RemoteTrainingEmissionMode::AppendOnlySnapshots
        );
        assert_eq!(
            outcome.bundle.result_classification,
            RemoteTrainingResultClassification::CompletedSuccess
        );
        assert_eq!(
            outcome.bundle.series_status,
            RemoteTrainingSeriesStatus::Available
        );
        assert_eq!(outcome.bundle.summary.total_steps_completed, 1);
        assert_eq!(outcome.bundle.loss_series.len(), 1);
        assert_eq!(outcome.bundle.distributed_series.len(), 1);
        assert!(outcome.bundle.heartbeat_series.len() >= 3);
        assert!(
            fs::read_dir(outcome.paths.snapshot_dir.as_path())?
                .filter_map(Result::ok)
                .count()
                >= outcome.bundle.refresh_contract.heartbeat_seq as usize
        );
        Ok(())
    }

    #[test]
    fn finalizer_report_preserves_live_series_and_emission_mode() -> Result<(), Box<dyn Error>> {
        let tempdir = tempdir()?;
        let run_root = tempdir.path().join("parameter-golf-live-run");
        fs::create_dir_all(run_root.as_path())?;
        let bringup_report_path = run_root.join("parameter_golf_distributed_8xh100_bringup.json");
        let runtime_manifest_path = run_root.join("runtime_manifest.json");
        fs::write(
            bringup_report_path.as_path(),
            "{\n  \"schema_version\": 1\n}\n",
        )?;
        fs::write(
            runtime_manifest_path.as_path(),
            "{\n  \"schema_version\": 1\n}\n",
        )?;
        let metadata = ParameterGolfDistributedVisualizationMetadata {
            provider: RemoteTrainingProvider::RunPod,
            profile_id: String::from("runpod_8xh100_parameter_golf"),
            lane_id: String::from("parameter_golf_distributed_8xh100"),
            repo_revision: String::from("workspace@test"),
            run_root: run_root.clone(),
            bringup_report_path: bringup_report_path.clone(),
            runtime_manifest_path: runtime_manifest_path.clone(),
        };
        let mut writer = ParameterGolfDistributedLiveVisualizationWriter::start(
            metadata,
            "parameter-golf-runpod-test",
            "The RunPod runtime started.",
        )?;
        let receipt = sample_train_step_receipt(run_root.as_path());
        writer.record_train_step_receipt(&receipt)?;
        let live_outcome = writer.finish(
            RemoteTrainingResultClassification::CompletedSuccess,
            "The runtime completed and sealed the retained live series.",
        )?;
        let inventory_path = run_root.join("nvidia_smi_inventory.txt");
        let topology_path = run_root.join("nvidia_smi_topology.txt");
        fs::write(
            inventory_path.as_path(),
            "0, NVIDIA H100 80GB HBM3, 81559 MiB, 1024 MiB, 76 %\n",
        )?;
        fs::write(topology_path.as_path(), "GPU0\n")?;
        let report_path = run_root.join("finalizer_report.json");
        let report = ParameterGolfRunPod8xH100FinalizerReport {
            schema_version: String::from("parameter_golf.runpod_8xh100_finalizer.v1"),
            runner: String::from("scripts/parameter-golf-runpod-finalize-8xh100.sh"),
            created_at_utc: Some(String::from("2026-03-25T14:00:00Z")),
            run_id: Some(String::from("parameter-golf-runpod-test")),
            profile_id: Some(String::from("runpod_8xh100_parameter_golf")),
            trainer_lane_id: Some(String::from("parameter_golf_distributed_8xh100")),
            run_root: run_root.display().to_string(),
            submission_dir: run_root.join("exported_submission").display().to_string(),
            world_size: 8,
            grad_accum_steps: 1,
            accelerator_evidence: ParameterGolfRunPod8xH100AcceleratorEvidence {
                inventory_path: inventory_path.display().to_string(),
                topology_path: topology_path.display().to_string(),
                inventory_line_count: 1,
            },
            exported_folder_evidence: ParameterGolfRunPod8xH100ExportedFolderEvidence {
                entrypoint_path: String::from("train_gpt.py"),
                entrypoint_sha256: Some(String::from("entrypoint-sha")),
                submission_manifest_path: String::from("submission.json"),
                submission_manifest_sha256: Some(String::from("manifest-sha")),
                submission_run_evidence_path: None,
                submission_run_evidence_sha256: None,
                distributed_receipt_path: Some(receipt.distributed_receipt_path.clone()),
                distributed_receipt_sha256: Some(
                    receipt.distributed_receipt.receipt_digest.clone(),
                ),
            },
            claim_boundary: String::from("claim boundary"),
        };
        fs::write(
            report_path.as_path(),
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        let outcome = write_parameter_golf_distributed_visualization_from_finalizer_report(
            report_path.as_path(),
            "workspace@test",
        )?;
        outcome.bundle.validate()?;
        outcome.run_index.validate()?;
        assert_eq!(
            outcome.bundle.refresh_contract.emission_mode,
            RemoteTrainingEmissionMode::AppendOnlySnapshots
        );
        assert_eq!(
            outcome.bundle.series_status,
            RemoteTrainingSeriesStatus::Available
        );
        assert!(
            outcome.bundle.heartbeat_series.len() >= live_outcome.bundle.heartbeat_series.len()
        );
        assert!(outcome.bundle.loss_series.len() >= live_outcome.bundle.loss_series.len());
        assert!(outcome
            .bundle
            .source_artifacts
            .iter()
            .any(|artifact| artifact.artifact_role == "finalizer_report"));
        assert!(outcome
            .bundle
            .source_artifacts
            .iter()
            .any(|artifact| artifact.artifact_role == "train_step_receipt"));
        assert!(outcome.paths.finalized_snapshot_path.is_file());
        Ok(())
    }
}
