use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS, RemoteTrainingArtifactSourceKind,
    RemoteTrainingEmissionMode, RemoteTrainingEventSample, RemoteTrainingEventSeverity,
    RemoteTrainingGpuSample, RemoteTrainingHeartbeatSample, RemoteTrainingLossSample,
    RemoteTrainingMathSample, RemoteTrainingProvider, RemoteTrainingRefreshContract,
    RemoteTrainingResultClassification, RemoteTrainingRunIndex, RemoteTrainingRunIndexEntry,
    RemoteTrainingRuntimeSample, RemoteTrainingSeriesStatus, RemoteTrainingSourceArtifact,
    RemoteTrainingTimelineEntry, RemoteTrainingVisualizationBundle,
    RemoteTrainingVisualizationError, RemoteTrainingVisualizationSummary,
    build_remote_training_run_index, record_remote_training_visualization_bundle,
};

pub const PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_DIR_NAME: &str = "training_visualization";
pub const PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_BUNDLE_NAME: &str =
    "psion_google_single_node_remote_training_visualization_bundle_v1.json";
pub const PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_RUN_INDEX_NAME: &str =
    "remote_training_run_index_v1.json";
pub const PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_SNAPSHOT_DIR_NAME: &str = "snapshots";
const PSION_GOOGLE_SINGLE_NODE_LIVE_STALE_AFTER_MS: u64 = 2_500;

const REMOTE_TRAINING_PROVIDER_ENV: &str = "PSIONIC_REMOTE_TRAINING_PROVIDER";
const REMOTE_TRAINING_PROFILE_ID_ENV: &str = "PSIONIC_REMOTE_TRAINING_PROFILE_ID";
const REMOTE_TRAINING_LANE_ID_ENV: &str = "PSIONIC_REMOTE_TRAINING_LANE_ID";
const REMOTE_TRAINING_REPO_REVISION_ENV: &str = "PSIONIC_REMOTE_TRAINING_REPO_REVISION";

#[derive(Debug, Error)]
pub enum PsionGoogleSingleNodeVisualizationError {
    #[error("psion google single-node visualization metadata is missing `{field}`")]
    MissingMetadata { field: String },
    #[error("psion google single-node visualization provider `{value}` is unsupported")]
    UnsupportedProvider { value: String },
    #[error("psion google single-node visualization could not read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("psion google single-node visualization could not write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("psion google single-node visualization could not decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("psion google single-node visualization background writer failed: {message}")]
    Background { message: String },
    #[error(transparent)]
    Contract(#[from] RemoteTrainingVisualizationError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionGoogleSingleNodeVisualizationMetadata {
    pub provider: RemoteTrainingProvider,
    pub profile_id: String,
    pub lane_id: String,
    pub repo_revision: String,
    pub output_dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PsionGoogleSingleNodeVisualizationPaths {
    pub bundle_path: PathBuf,
    pub run_index_path: PathBuf,
    pub snapshot_dir: PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PsionGoogleSingleNodeStepTelemetry {
    pub global_step: u64,
    pub train_loss: Option<f32>,
    pub ema_loss: Option<f32>,
    pub validation_loss: Option<f32>,
    pub learning_rate: Option<f32>,
    pub gradient_norm: Option<f32>,
    pub parameter_norm: Option<f32>,
    pub update_norm: Option<f32>,
    pub clip_fraction: Option<f32>,
    pub clip_event_count: Option<u32>,
    pub non_finite_count: u32,
    pub model_specific_diagnostics: BTreeMap<String, f32>,
    pub data_wait_ms: Option<u64>,
    pub forward_ms: Option<u64>,
    pub backward_ms: Option<u64>,
    pub optimizer_ms: Option<u64>,
    pub checkpoint_ms: Option<u64>,
    pub evaluation_ms: Option<u64>,
    pub tokens_per_second: Option<u64>,
    pub samples_per_second_milli: Option<u32>,
    pub active_subsystems: Vec<String>,
    pub summary_detail: String,
}

#[derive(Clone, Debug, PartialEq)]
struct PsionGoogleSingleNodeVisualizationState {
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
    event_series: Vec<RemoteTrainingEventSample>,
    source_artifacts: BTreeMap<String, RemoteTrainingSourceArtifact>,
}

struct PsionGoogleSingleNodeVisualizationShared {
    state: PsionGoogleSingleNodeVisualizationState,
    last_flush_at_ms: Option<u64>,
    background_error: Option<String>,
}

pub struct PsionGoogleSingleNodeLiveVisualizationWriter {
    metadata: PsionGoogleSingleNodeVisualizationMetadata,
    paths: PsionGoogleSingleNodeVisualizationPaths,
    shared: Arc<Mutex<PsionGoogleSingleNodeVisualizationShared>>,
    stop_requested: Arc<AtomicBool>,
    background_thread: Option<JoinHandle<()>>,
}

impl PsionGoogleSingleNodeVisualizationMetadata {
    pub fn from_runtime_env(
        output_dir: &Path,
    ) -> Result<Option<Self>, PsionGoogleSingleNodeVisualizationError> {
        let Some(provider_value) = env::var_os(REMOTE_TRAINING_PROVIDER_ENV) else {
            return Ok(None);
        };
        let provider_raw = provider_value.to_string_lossy().trim().to_string();
        let provider = parse_provider(provider_raw.as_str())?;
        if provider != RemoteTrainingProvider::GoogleCloud {
            return Ok(None);
        }
        Ok(Some(Self {
            provider,
            profile_id: required_env(REMOTE_TRAINING_PROFILE_ID_ENV)?,
            lane_id: required_env(REMOTE_TRAINING_LANE_ID_ENV)?,
            repo_revision: env::var(REMOTE_TRAINING_REPO_REVISION_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| String::from("workspace@unknown")),
            output_dir: output_dir.to_path_buf(),
        }))
    }

    #[must_use]
    pub fn new(
        profile_id: impl Into<String>,
        lane_id: impl Into<String>,
        repo_revision: impl Into<String>,
        output_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            provider: RemoteTrainingProvider::GoogleCloud,
            profile_id: profile_id.into(),
            lane_id: lane_id.into(),
            repo_revision: repo_revision.into(),
            output_dir: output_dir.into(),
        }
    }

    #[must_use]
    pub fn paths(&self) -> PsionGoogleSingleNodeVisualizationPaths {
        let visualization_dir = self
            .output_dir
            .join(PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_DIR_NAME);
        PsionGoogleSingleNodeVisualizationPaths {
            bundle_path: visualization_dir.join(PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_BUNDLE_NAME),
            run_index_path: visualization_dir
                .join(PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_RUN_INDEX_NAME),
            snapshot_dir: visualization_dir
                .join(PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_SNAPSHOT_DIR_NAME),
        }
    }

    #[must_use]
    pub fn bundle_relative_path(&self) -> String {
        format!(
            "{}/{}",
            PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_DIR_NAME,
            PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_BUNDLE_NAME
        )
    }

    #[must_use]
    pub fn run_index_relative_path(&self) -> String {
        format!(
            "{}/{}",
            PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_DIR_NAME,
            PSION_GOOGLE_SINGLE_NODE_VISUALIZATION_RUN_INDEX_NAME
        )
    }
}

impl PsionGoogleSingleNodeLiveVisualizationWriter {
    pub fn try_start(
        output_dir: &Path,
        run_id: &str,
        initial_detail: impl Into<String>,
    ) -> Result<Option<Self>, PsionGoogleSingleNodeVisualizationError> {
        let Some(metadata) =
            PsionGoogleSingleNodeVisualizationMetadata::from_runtime_env(output_dir)?
        else {
            return Ok(None);
        };
        Self::start(metadata, run_id, initial_detail).map(Some)
    }

    pub fn start(
        metadata: PsionGoogleSingleNodeVisualizationMetadata,
        run_id: &str,
        initial_detail: impl Into<String>,
    ) -> Result<Self, PsionGoogleSingleNodeVisualizationError> {
        let paths = metadata.paths();
        fs::create_dir_all(
            paths
                .bundle_path
                .parent()
                .expect("bundle path should have a parent"),
        )
        .map_err(|error| PsionGoogleSingleNodeVisualizationError::Write {
            path: metadata.output_dir.display().to_string(),
            error,
        })?;
        fs::create_dir_all(paths.snapshot_dir.as_path()).map_err(|error| {
            PsionGoogleSingleNodeVisualizationError::Write {
                path: paths.snapshot_dir.display().to_string(),
                error,
            }
        })?;
        let started_at_ms = unix_time_ms();
        let initial_detail = initial_detail.into();
        let shared = Arc::new(Mutex::new(PsionGoogleSingleNodeVisualizationShared {
            state: PsionGoogleSingleNodeVisualizationState {
                run_id: run_id.to_string(),
                result_classification: RemoteTrainingResultClassification::Active,
                started_at_ms,
                finished_at_ms: None,
                phase: String::from("provisioning"),
                subphase: Some(String::from("trainer_boot")),
                step_in_progress: None,
                active_subsystems: vec![String::from("trainer_boot")],
                summary_detail: initial_detail.clone(),
                heartbeat_seq: 0,
                total_steps_completed: 0,
                latest_checkpoint_ref: None,
                timeline: vec![RemoteTrainingTimelineEntry {
                    observed_at_ms: started_at_ms,
                    phase: String::from("provisioning"),
                    subphase: Some(String::from("trainer_boot")),
                    detail: initial_detail.clone(),
                }],
                heartbeat_series: Vec::new(),
                loss_series: Vec::new(),
                math_series: Vec::new(),
                runtime_series: Vec::new(),
                gpu_series: Vec::new(),
                event_series: vec![RemoteTrainingEventSample {
                    observed_at_ms: started_at_ms,
                    severity: RemoteTrainingEventSeverity::Info,
                    event_kind: String::from("trainer_started"),
                    detail: initial_detail,
                }],
                source_artifacts: BTreeMap::new(),
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
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
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
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
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

    pub fn record_step(
        &mut self,
        telemetry: PsionGoogleSingleNodeStepTelemetry,
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        self.with_shared_mut(|shared| {
            let observed_at_ms = unix_time_ms();
            shared.state.total_steps_completed = telemetry.global_step;
            shared.state.step_in_progress = None;
            if !telemetry.active_subsystems.is_empty() {
                shared.state.active_subsystems = dedup_strings(telemetry.active_subsystems.clone());
            }
            shared.state.summary_detail = telemetry.summary_detail.clone();
            shared.state.loss_series.push(RemoteTrainingLossSample {
                global_step: Some(telemetry.global_step),
                elapsed_ms: observed_at_ms.saturating_sub(shared.state.started_at_ms),
                train_loss: telemetry.train_loss,
                ema_loss: telemetry.ema_loss,
                validation_loss: telemetry.validation_loss,
            });
            shared.state.math_series.push(RemoteTrainingMathSample {
                observed_at_ms,
                global_step: Some(telemetry.global_step),
                learning_rate: telemetry.learning_rate,
                gradient_norm: telemetry.gradient_norm,
                parameter_norm: telemetry.parameter_norm,
                update_norm: telemetry.update_norm,
                clip_fraction: telemetry.clip_fraction,
                clip_event_count: telemetry.clip_event_count,
                loss_scale: None,
                non_finite_count: telemetry.non_finite_count,
                model_specific_diagnostics: telemetry.model_specific_diagnostics,
            });
            shared
                .state
                .runtime_series
                .push(RemoteTrainingRuntimeSample {
                    observed_at_ms,
                    data_wait_ms: telemetry.data_wait_ms,
                    forward_ms: telemetry.forward_ms,
                    backward_ms: telemetry.backward_ms,
                    optimizer_ms: telemetry.optimizer_ms,
                    checkpoint_ms: telemetry.checkpoint_ms,
                    evaluation_ms: telemetry.evaluation_ms,
                    tokens_per_second: telemetry.tokens_per_second,
                    samples_per_second_milli: telemetry.samples_per_second_milli,
                });
            Ok(())
        })?;
        self.flush(false)
    }

    pub fn record_validation_loss(
        &mut self,
        global_step: Option<u64>,
        validation_loss: f32,
        detail: impl Into<String>,
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        self.with_shared_mut(|shared| {
            let observed_at_ms = unix_time_ms();
            let detail = detail.into();
            shared.state.summary_detail = detail.clone();
            shared.state.loss_series.push(RemoteTrainingLossSample {
                global_step,
                elapsed_ms: observed_at_ms.saturating_sub(shared.state.started_at_ms),
                train_loss: None,
                ema_loss: None,
                validation_loss: Some(validation_loss),
            });
            shared.state.event_series.push(RemoteTrainingEventSample {
                observed_at_ms,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("validation_checkpoint"),
                detail,
            });
            Ok(())
        })?;
        self.flush(false)
    }

    pub fn record_checkpoint_ref(
        &mut self,
        checkpoint_ref: impl Into<String>,
        checkpoint_ms: Option<u64>,
        detail: impl Into<String>,
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        self.with_shared_mut(|shared| {
            let observed_at_ms = unix_time_ms();
            let checkpoint_ref = checkpoint_ref.into();
            let detail = detail.into();
            shared.state.latest_checkpoint_ref = Some(checkpoint_ref.clone());
            shared.state.summary_detail = detail.clone();
            shared
                .state
                .runtime_series
                .push(RemoteTrainingRuntimeSample {
                    observed_at_ms,
                    data_wait_ms: None,
                    forward_ms: None,
                    backward_ms: None,
                    optimizer_ms: None,
                    checkpoint_ms,
                    evaluation_ms: None,
                    tokens_per_second: None,
                    samples_per_second_milli: None,
                });
            shared.state.event_series.push(RemoteTrainingEventSample {
                observed_at_ms,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("checkpoint_written"),
                detail,
            });
            Ok(())
        })?;
        self.flush(true)
    }

    pub fn record_source_artifact(
        &mut self,
        artifact_role: impl Into<String>,
        artifact_uri: impl Into<String>,
        artifact_digest: Option<String>,
        source_kind: RemoteTrainingArtifactSourceKind,
        authoritative: bool,
        source_receipt_ids: Vec<String>,
        detail: impl Into<String>,
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        self.with_shared_mut(|shared| {
            let artifact_role = artifact_role.into();
            shared.state.source_artifacts.insert(
                artifact_role.clone(),
                RemoteTrainingSourceArtifact {
                    artifact_role,
                    artifact_uri: artifact_uri.into(),
                    artifact_digest,
                    source_kind,
                    authoritative,
                    source_receipt_ids,
                    detail: detail.into(),
                },
            );
            Ok(())
        })?;
        self.flush(true)
    }

    pub fn finish_success(
        &mut self,
        summary_detail: impl Into<String>,
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        self.finish(
            RemoteTrainingResultClassification::CompletedSuccess,
            summary_detail,
        )
    }

    pub fn finish_failure(
        &mut self,
        summary_detail: impl Into<String>,
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        self.finish(
            RemoteTrainingResultClassification::CompletedFailure,
            summary_detail,
        )
    }

    pub fn flush(&mut self, force: bool) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        let metadata = self.metadata.clone();
        let paths = self.paths.clone();
        self.with_shared_mut(|shared| flush_locked(&metadata, &paths, shared, force))
    }

    fn finish(
        &mut self,
        result_classification: RemoteTrainingResultClassification,
        summary_detail: impl Into<String>,
    ) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        let metadata = self.metadata.clone();
        let paths = self.paths.clone();
        self.with_shared_mut(|shared| {
            let observed_at_ms = unix_time_ms();
            let summary_detail = summary_detail.into();
            shared.state.result_classification = result_classification;
            shared.state.finished_at_ms = Some(observed_at_ms);
            shared.state.phase = String::from("complete");
            shared.state.subphase = Some(match result_classification {
                RemoteTrainingResultClassification::CompletedSuccess => {
                    String::from("completed_success")
                }
                RemoteTrainingResultClassification::CompletedFailure => {
                    String::from("completed_failure")
                }
                _ => String::from("sealed"),
            });
            shared.state.step_in_progress = None;
            shared.state.active_subsystems = vec![String::from("artifact_seal")];
            shared.state.summary_detail = summary_detail.clone();
            shared.state.timeline.push(RemoteTrainingTimelineEntry {
                observed_at_ms,
                phase: shared.state.phase.clone(),
                subphase: shared.state.subphase.clone(),
                detail: summary_detail.clone(),
            });
            shared.state.event_series.push(RemoteTrainingEventSample {
                observed_at_ms,
                severity: if result_classification
                    == RemoteTrainingResultClassification::CompletedFailure
                {
                    RemoteTrainingEventSeverity::Error
                } else {
                    RemoteTrainingEventSeverity::Info
                },
                event_kind: String::from("trainer_finished"),
                detail: summary_detail,
            });
            flush_locked(&metadata, &paths, shared, true)
        })?;
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Some(handle) = self.background_thread.take() {
            let _ = handle.join();
        }
        self.check_background_error()
    }

    fn with_shared_mut<T>(
        &mut self,
        op: impl FnOnce(
            &mut PsionGoogleSingleNodeVisualizationShared,
        ) -> Result<T, PsionGoogleSingleNodeVisualizationError>,
    ) -> Result<T, PsionGoogleSingleNodeVisualizationError> {
        let result = {
            let mut shared = self.shared.lock().expect("writer shared state should lock");
            if let Some(message) = shared.background_error.clone() {
                return Err(PsionGoogleSingleNodeVisualizationError::Background { message });
            }
            op(&mut shared)?
        };
        self.check_background_error()?;
        Ok(result)
    }

    fn check_background_error(&self) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
        let shared = self.shared.lock().expect("writer shared state should lock");
        if let Some(message) = shared.background_error.clone() {
            return Err(PsionGoogleSingleNodeVisualizationError::Background { message });
        }
        Ok(())
    }
}

impl Drop for PsionGoogleSingleNodeLiveVisualizationWriter {
    fn drop(&mut self) {
        self.stop_requested.store(true, Ordering::SeqCst);
        if let Some(handle) = self.background_thread.take() {
            let _ = handle.join();
        }
    }
}

fn spawn_background_writer(
    metadata: PsionGoogleSingleNodeVisualizationMetadata,
    paths: PsionGoogleSingleNodeVisualizationPaths,
    shared: Arc<Mutex<PsionGoogleSingleNodeVisualizationShared>>,
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
            if let Err(error) = flush_locked(&metadata, &paths, &mut shared, false) {
                shared.background_error = Some(error.to_string());
                stop_requested.store(true, Ordering::SeqCst);
                break;
            }
        }
    })
}

fn flush_locked(
    metadata: &PsionGoogleSingleNodeVisualizationMetadata,
    paths: &PsionGoogleSingleNodeVisualizationPaths,
    shared: &mut PsionGoogleSingleNodeVisualizationShared,
    force: bool,
) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
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
            stale_after_ms: PSION_GOOGLE_SINGLE_NODE_LIVE_STALE_AFTER_MS,
        });
    shared
        .state
        .gpu_series
        .extend(collect_local_gpu_samples(observed_at_ms));
    let bundle = build_bundle_from_state(metadata, &shared.state)?;
    let run_index = build_run_index(metadata, &bundle)?;
    write_atomically_json(paths.bundle_path.as_path(), &bundle)?;
    write_atomically_json(paths.run_index_path.as_path(), &run_index)?;
    let snapshot_path = paths
        .snapshot_dir
        .join(format!("heartbeat_{:08}.json", shared.state.heartbeat_seq));
    write_atomically_json(snapshot_path.as_path(), &bundle)?;
    shared.last_flush_at_ms = Some(observed_at_ms);
    Ok(())
}

fn build_bundle_from_state(
    metadata: &PsionGoogleSingleNodeVisualizationMetadata,
    state: &PsionGoogleSingleNodeVisualizationState,
) -> Result<RemoteTrainingVisualizationBundle, PsionGoogleSingleNodeVisualizationError> {
    let (series_status, series_unavailable_reason) = if state.loss_series.is_empty() {
        let reason = if state.result_classification == RemoteTrainingResultClassification::Active {
            String::from(
                "the Google single-node lane has not emitted its first optimizer-step or validation sample yet",
            )
        } else {
            String::from(
                "the Google single-node lane finished without any retained optimizer-step or validation samples",
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
            bundle_id: format!("{}-google-single-node-remote-training-v1", state.run_id),
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
                latest_samples_per_second_milli: latest_runtime
                    .as_ref()
                    .and_then(|sample| sample.samples_per_second_milli),
                accumulated_cost_microusd: None,
                latest_checkpoint_ref: state.latest_checkpoint_ref.clone(),
                detail: state.summary_detail.clone(),
            },
            heartbeat_series: state.heartbeat_series.clone(),
            loss_series: state.loss_series.clone(),
            math_series: state.math_series.clone(),
            runtime_series: state.runtime_series.clone(),
            gpu_series: state.gpu_series.clone(),
            distributed_series: Vec::new(),
            event_series: state.event_series.clone(),
            source_artifacts: build_source_artifacts(metadata, state),
            bundle_digest: String::new(),
        },
    )?)
}

fn build_run_index(
    metadata: &PsionGoogleSingleNodeVisualizationMetadata,
    bundle: &RemoteTrainingVisualizationBundle,
) -> Result<RemoteTrainingRunIndex, PsionGoogleSingleNodeVisualizationError> {
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
            bundle_artifact_uri: Some(metadata.bundle_relative_path()),
            bundle_digest: Some(bundle.bundle_digest.clone()),
            summary_label: format!("Google {} {}", metadata.profile_id, metadata.lane_id),
            detail: bundle.summary.detail.clone(),
        }],
        detail: String::from(
            "This run index enumerates the live or finalized Google single-node Psion visualization bundle for the app mirror.",
        ),
        index_digest: String::new(),
    })?)
}

fn build_source_artifacts(
    metadata: &PsionGoogleSingleNodeVisualizationMetadata,
    state: &PsionGoogleSingleNodeVisualizationState,
) -> Vec<RemoteTrainingSourceArtifact> {
    let mut artifacts = vec![
        RemoteTrainingSourceArtifact {
            artifact_role: String::from("live_bundle"),
            artifact_uri: metadata.bundle_relative_path(),
            artifact_digest: None,
            source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
            authoritative: true,
            source_receipt_ids: vec![String::from(
                "psion.google_single_node.remote_training_live_bundle.v1",
            )],
            detail: String::from(
                "The provider-neutral local mirror is the app-facing source for this Google single-node lane.",
            ),
        },
        RemoteTrainingSourceArtifact {
            artifact_role: String::from("run_index"),
            artifact_uri: metadata.run_index_relative_path(),
            artifact_digest: None,
            source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
            authoritative: false,
            source_receipt_ids: vec![String::from(
                "psion.google_single_node.remote_training_run_index.v1",
            )],
            detail: String::from(
                "The run index enumerates this Google single-node bundle for normalized app discovery.",
            ),
        },
    ];
    artifacts.extend(state.source_artifacts.values().cloned());
    artifacts
}

fn parse_provider(
    raw: &str,
) -> Result<RemoteTrainingProvider, PsionGoogleSingleNodeVisualizationError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "google_cloud" | "googlecloud" | "google" => Ok(RemoteTrainingProvider::GoogleCloud),
        value => Err(
            PsionGoogleSingleNodeVisualizationError::UnsupportedProvider {
                value: value.to_string(),
            },
        ),
    }
}

fn required_env(name: &str) -> Result<String, PsionGoogleSingleNodeVisualizationError> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(
            || PsionGoogleSingleNodeVisualizationError::MissingMetadata {
                field: name.to_string(),
            },
        )
}

fn write_atomically_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PsionGoogleSingleNodeVisualizationError> {
    let parent = path
        .parent()
        .expect("json output path should have a parent");
    fs::create_dir_all(parent).map_err(|error| PsionGoogleSingleNodeVisualizationError::Write {
        path: parent.display().to_string(),
        error,
    })?;
    let tmp_path = path.with_extension("tmp");
    let encoded = serde_json::to_vec_pretty(value).map_err(|error| {
        PsionGoogleSingleNodeVisualizationError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })?;
    fs::write(
        tmp_path.as_path(),
        format!("{}\n", String::from_utf8_lossy(&encoded)),
    )
    .map_err(|error| PsionGoogleSingleNodeVisualizationError::Write {
        path: tmp_path.display().to_string(),
        error,
    })?;
    fs::rename(tmp_path.as_path(), path).map_err(|error| {
        PsionGoogleSingleNodeVisualizationError::Write {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(())
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeMap::<String, ()>::new();
    let mut deduped = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        if seen.insert(trimmed.to_string(), ()).is_none() {
            deduped.push(trimmed.to_string());
        }
    }
    deduped
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
    let raw = String::from_utf8_lossy(&output.stdout);
    raw.lines()
        .filter_map(|line| {
            let parts = line.split(',').map(|part| part.trim()).collect::<Vec<_>>();
            if parts.len() < 5 {
                return None;
            }
            let device_index = parts.first()?.parse::<u32>().ok()?;
            let device_label = parts.get(1)?.to_string();
            let utilization_percent = parts.get(2)?.parse::<u32>().ok()?;
            let memory_used_mib = parts.get(3)?.parse::<u64>().ok()?;
            let memory_total_mib = parts.get(4)?.parse::<u64>().ok()?;
            let temperature_celsius = parts.get(5).and_then(|value| value.parse::<u16>().ok());
            let power_watts = parts.get(6).and_then(|value| {
                let whole = value.split('.').next().unwrap_or(value);
                whole.parse::<u16>().ok()
            });
            Some(RemoteTrainingGpuSample {
                observed_at_ms,
                device_id: format!("cuda:{device_index}"),
                device_label,
                utilization_bps: utilization_percent.saturating_mul(100),
                memory_used_bytes: memory_used_mib.saturating_mul(1024 * 1024),
                memory_total_bytes: memory_total_mib.saturating_mul(1024 * 1024),
                temperature_celsius,
                power_watts,
            })
        })
        .collect()
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

    #[test]
    fn writer_emits_live_google_bundle_with_step_series() -> Result<(), Box<dyn Error>> {
        let tempdir = tempdir()?;
        let output_dir = tempdir.path().join("output");
        fs::create_dir_all(output_dir.as_path())?;
        let metadata = PsionGoogleSingleNodeVisualizationMetadata::new(
            "g2_l4_single_node_accelerated",
            "psion_accelerated_reference_pilot",
            "workspace@test",
            output_dir.clone(),
        );
        let mut writer = PsionGoogleSingleNodeLiveVisualizationWriter::start(
            metadata.clone(),
            "psion-google-live-test",
            "The Google single-node accelerated lane started.",
        )?;
        writer.record_phase(
            "training",
            Some(String::from("optimizer_step")),
            "The accelerated lane entered measured optimizer steps.",
            vec![String::from("cuda_graph"), String::from("optimizer_step")],
            Some(1),
            true,
        )?;
        writer.record_step(PsionGoogleSingleNodeStepTelemetry {
            global_step: 1,
            train_loss: Some(2.5),
            learning_rate: Some(0.0005),
            gradient_norm: Some(1.25),
            parameter_norm: Some(9.0),
            update_norm: Some(0.18),
            clip_fraction: Some(0.2),
            clip_event_count: Some(1),
            data_wait_ms: None,
            forward_ms: Some(800),
            optimizer_ms: Some(200),
            tokens_per_second: Some(51_200),
            samples_per_second_milli: Some(8_000),
            active_subsystems: vec![String::from("optimizer_step")],
            summary_detail: String::from(
                "The accelerated lane completed optimizer step 1 and retained live telemetry.",
            ),
            ..PsionGoogleSingleNodeStepTelemetry::default()
        })?;
        writer.record_validation_loss(
            Some(1),
            2.1,
            "The accelerated lane retained a validation checkpoint after step 1.",
        )?;
        writer.record_checkpoint_ref(
            "checkpoint://psion/reference/1",
            Some(120),
            "The accelerated lane retained its promoted checkpoint ref.",
        )?;
        writer.record_source_artifact(
            "stage_receipt",
            "psion_accelerated_reference_pilot_stage_receipt.json",
            Some(String::from("receipt-digest")),
            RemoteTrainingArtifactSourceKind::RuntimeOwned,
            true,
            vec![String::from("receipt.psion.pretrain_stage.v1")],
            "The stage receipt remains authoritative for the accelerated execution posture.",
        )?;
        writer.finish_success(
            "The Google single-node accelerated lane completed and sealed its live bundle.",
        )?;

        let bundle: RemoteTrainingVisualizationBundle =
            serde_json::from_str(fs::read_to_string(metadata.paths().bundle_path)?.as_str())?;
        let run_index: RemoteTrainingRunIndex =
            serde_json::from_str(fs::read_to_string(metadata.paths().run_index_path)?.as_str())?;
        bundle.validate()?;
        run_index.validate()?;
        assert_eq!(
            bundle.result_classification,
            RemoteTrainingResultClassification::CompletedSuccess
        );
        assert_eq!(bundle.series_status, RemoteTrainingSeriesStatus::Available);
        assert_eq!(bundle.summary.total_steps_completed, 1);
        assert_eq!(
            bundle.summary.latest_checkpoint_ref.as_deref(),
            Some("checkpoint://psion/reference/1")
        );
        assert!(metadata.paths().snapshot_dir.is_dir());
        assert!(
            !fs::read_dir(metadata.paths().snapshot_dir)?
                .collect::<Result<Vec<_>, _>>()?
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn writer_finishes_failure_without_series() -> Result<(), Box<dyn Error>> {
        let tempdir = tempdir()?;
        let output_dir = tempdir.path().join("output");
        fs::create_dir_all(output_dir.as_path())?;
        let metadata = PsionGoogleSingleNodeVisualizationMetadata::new(
            "g2_l4_single_node_plugin_host_native_accelerated",
            "psion_plugin_host_native_accelerated",
            "workspace@test",
            output_dir.clone(),
        );
        let mut writer = PsionGoogleSingleNodeLiveVisualizationWriter::start(
            metadata.clone(),
            "psion-google-failure-test",
            "The Google plugin-conditioned lane started.",
        )?;
        writer.finish_failure(
            "The Google plugin-conditioned lane failed before the first optimizer step.",
        )?;
        let bundle: RemoteTrainingVisualizationBundle =
            serde_json::from_str(fs::read_to_string(metadata.paths().bundle_path)?.as_str())?;
        bundle.validate()?;
        assert_eq!(
            bundle.result_classification,
            RemoteTrainingResultClassification::CompletedFailure
        );
        assert_eq!(
            bundle.series_status,
            RemoteTrainingSeriesStatus::Unavailable
        );
        Ok(())
    }
}
