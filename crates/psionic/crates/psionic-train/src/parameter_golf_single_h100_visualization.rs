use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    build_remote_training_run_index, record_remote_training_visualization_bundle,
    ParameterGolfBatchGeometry, ParameterGolfSingleH100RoundtripReceipt,
    ParameterGolfSingleH100TrainingReport, ParameterGolfSingleH100TrainingStepMetrics,
    ParameterGolfSingleH100TrainingStopReason, ParameterGolfSingleH100ValidationCheckpoint,
    ParameterGolfSingleH100ValidationSummary, ParameterGolfValidationEvalMode,
    RemoteTrainingArtifactSourceKind, RemoteTrainingEmissionMode, RemoteTrainingEventSample,
    RemoteTrainingEventSeverity, RemoteTrainingGpuSample, RemoteTrainingHeartbeatSample,
    RemoteTrainingLossSample, RemoteTrainingMathSample, RemoteTrainingProvider,
    RemoteTrainingRefreshContract, RemoteTrainingResultClassification, RemoteTrainingRunIndex,
    RemoteTrainingRunIndexEntry, RemoteTrainingRuntimeSample, RemoteTrainingSeriesStatus,
    RemoteTrainingSourceArtifact, RemoteTrainingTimelineEntry, RemoteTrainingVisualizationBundle,
    RemoteTrainingVisualizationError, RemoteTrainingVisualizationSummary,
    REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
};

pub const PARAMETER_GOLF_SINGLE_H100_VISUALIZATION_DIR_NAME: &str = "training_visualization";
pub const PARAMETER_GOLF_SINGLE_H100_VISUALIZATION_BUNDLE_NAME: &str =
    "parameter_golf_single_h100_remote_training_visualization_bundle_v1.json";
pub const PARAMETER_GOLF_SINGLE_H100_VISUALIZATION_RUN_INDEX_NAME: &str =
    "remote_training_run_index_v1.json";
const PARAMETER_GOLF_SINGLE_H100_LIVE_STALE_AFTER_MS: u64 = 2_500;

const REMOTE_TRAINING_PROVIDER_ENV: &str = "PSIONIC_REMOTE_TRAINING_PROVIDER";
const REMOTE_TRAINING_PROFILE_ID_ENV: &str = "PSIONIC_REMOTE_TRAINING_PROFILE_ID";
const REMOTE_TRAINING_LANE_ID_ENV: &str = "PSIONIC_REMOTE_TRAINING_LANE_ID";
const REMOTE_TRAINING_REPO_REVISION_ENV: &str = "PSIONIC_REMOTE_TRAINING_REPO_REVISION";
const RUNPOD_POD_ID_ENV: &str = "RUNPOD_POD_ID";
const RUNPOD_PUBLIC_IP_ENV: &str = "RUNPOD_PUBLIC_IP";

#[derive(Debug, Error)]
pub enum ParameterGolfSingleH100VisualizationError {
    #[error("parameter golf single-H100 visualization metadata is missing `{field}`")]
    MissingMetadata { field: String },
    #[error("parameter golf single-H100 visualization provider `{value}` is unsupported")]
    UnsupportedProvider { value: String },
    #[error("parameter golf single-H100 visualization could not read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("parameter golf single-H100 visualization could not write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("parameter golf single-H100 visualization could not decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("parameter golf single-H100 visualization could not encode json: {message}")]
    Serialize { message: String },
    #[error("parameter golf single-H100 visualization log fallback is missing usable step truth")]
    MissingLogFallbackSeries,
    #[error(transparent)]
    RemoteTraining(#[from] RemoteTrainingVisualizationError),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterGolfSingleH100VisualizationMetadata {
    pub provider: RemoteTrainingProvider,
    pub profile_id: String,
    pub lane_id: String,
    pub repo_revision: String,
    pub run_root: PathBuf,
    pub report_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterGolfSingleH100VisualizationPaths {
    pub bundle_path: PathBuf,
    pub run_index_path: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterGolfSingleH100VisualizationWriteOutcome {
    pub bundle: RemoteTrainingVisualizationBundle,
    pub run_index: RemoteTrainingRunIndex,
    pub paths: ParameterGolfSingleH100VisualizationPaths,
}

#[derive(Clone, Debug, PartialEq)]
struct ParameterGolfSingleH100VisualizationState {
    run_id: String,
    result_classification: RemoteTrainingResultClassification,
    started_at_ms: u64,
    finished_at_ms: Option<u64>,
    phase: String,
    subphase: Option<String>,
    step_in_progress: Option<u64>,
    microbatch_in_progress: Option<u32>,
    active_subsystems: Vec<String>,
    summary_detail: String,
    heartbeat_seq: u64,
    timeline: Vec<RemoteTrainingTimelineEntry>,
    event_series: Vec<RemoteTrainingEventSample>,
    heartbeat_series: Vec<RemoteTrainingHeartbeatSample>,
    gpu_series: Vec<RemoteTrainingGpuSample>,
    step_metrics: Vec<ParameterGolfSingleH100TrainingStepMetrics>,
    validation_checkpoints: Vec<ParameterGolfSingleH100ValidationCheckpoint>,
    initial_validation: Option<ParameterGolfSingleH100ValidationSummary>,
    pre_export_final_validation: Option<ParameterGolfSingleH100ValidationSummary>,
    final_validation: Option<ParameterGolfSingleH100ValidationSummary>,
    final_roundtrip_receipt: Option<ParameterGolfSingleH100RoundtripReceipt>,
    training_report_digest: Option<String>,
    used_log_fallback: bool,
}

pub struct ParameterGolfSingleH100LiveVisualizationWriter {
    metadata: ParameterGolfSingleH100VisualizationMetadata,
    paths: ParameterGolfSingleH100VisualizationPaths,
    geometry: ParameterGolfBatchGeometry,
    state: ParameterGolfSingleH100VisualizationState,
    last_flush_at_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct GradientClipObservation {
    pub(crate) gradient_norm_after_clip: Option<f32>,
    pub(crate) clip_applied: bool,
    pub(crate) non_finite_count: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct LogFallbackStep {
    global_step: u64,
    mean_microbatch_loss: Option<f32>,
    learning_rate_multiplier: Option<f32>,
    muon_momentum: Option<f32>,
    forward_ms: Option<u64>,
    backward_ms: Option<u64>,
    host_materialization_ms: Option<u64>,
    optimizer_step_ms: Option<u64>,
}

impl ParameterGolfSingleH100VisualizationMetadata {
    pub fn from_runtime_env(
        run_id: &str,
        report_path: &Path,
    ) -> Result<Option<Self>, ParameterGolfSingleH100VisualizationError> {
        let Some(run_root) = report_path.parent() else {
            return Ok(None);
        };
        if let Some(provider_value) = env::var_os(REMOTE_TRAINING_PROVIDER_ENV) {
            let provider_raw = provider_value.to_string_lossy().trim().to_string();
            let provider = parse_provider(provider_raw.as_str())?;
            let profile_id = required_env(REMOTE_TRAINING_PROFILE_ID_ENV)?;
            let lane_id = required_env(REMOTE_TRAINING_LANE_ID_ENV)?;
            let repo_revision = env::var(REMOTE_TRAINING_REPO_REVISION_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| String::from("workspace@unknown"));
            return Ok(Some(Self {
                provider,
                profile_id,
                lane_id,
                repo_revision,
                run_root: run_root.to_path_buf(),
                report_path: report_path.to_path_buf(),
            }));
        }
        if env::var_os(RUNPOD_POD_ID_ENV).is_some() || env::var_os(RUNPOD_PUBLIC_IP_ENV).is_some() {
            return Ok(Some(Self {
                provider: RemoteTrainingProvider::RunPod,
                profile_id: String::from("runpod_h100_single_gpu"),
                lane_id: String::from("parameter_golf.runpod_single_h100"),
                repo_revision: String::from("workspace@unknown"),
                run_root: run_root.to_path_buf(),
                report_path: report_path.to_path_buf(),
            }));
        }
        let _ = run_id;
        Ok(None)
    }

    pub fn new(
        provider: RemoteTrainingProvider,
        profile_id: impl Into<String>,
        lane_id: impl Into<String>,
        repo_revision: impl Into<String>,
        run_root: impl Into<PathBuf>,
        report_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            provider,
            profile_id: profile_id.into(),
            lane_id: lane_id.into(),
            repo_revision: repo_revision.into(),
            run_root: run_root.into(),
            report_path: report_path.into(),
        }
    }

    pub fn paths(&self) -> ParameterGolfSingleH100VisualizationPaths {
        let visualization_dir = self
            .run_root
            .join(PARAMETER_GOLF_SINGLE_H100_VISUALIZATION_DIR_NAME);
        ParameterGolfSingleH100VisualizationPaths {
            bundle_path: visualization_dir
                .join(PARAMETER_GOLF_SINGLE_H100_VISUALIZATION_BUNDLE_NAME),
            run_index_path: visualization_dir
                .join(PARAMETER_GOLF_SINGLE_H100_VISUALIZATION_RUN_INDEX_NAME),
        }
    }

    pub fn bundle_relative_path(&self) -> String {
        format!(
            "{}/{}",
            PARAMETER_GOLF_SINGLE_H100_VISUALIZATION_DIR_NAME,
            PARAMETER_GOLF_SINGLE_H100_VISUALIZATION_BUNDLE_NAME
        )
    }

    pub fn report_relative_path(&self) -> String {
        relative_path_string(self.run_root.as_path(), self.report_path.as_path())
            .unwrap_or_else(|| self.report_path.display().to_string())
    }
}

impl ParameterGolfSingleH100LiveVisualizationWriter {
    pub fn try_start(
        config: &crate::ParameterGolfSingleH100TrainingConfig,
        report_path: &Path,
        started_at_ms: u64,
    ) -> Result<Option<Self>, ParameterGolfSingleH100VisualizationError> {
        let Some(metadata) = ParameterGolfSingleH100VisualizationMetadata::from_runtime_env(
            config.run_id.as_str(),
            report_path,
        )?
        else {
            return Ok(None);
        };
        let paths = metadata.paths();
        fs::create_dir_all(
            paths
                .bundle_path
                .parent()
                .expect("bundle path should have a parent"),
        )
        .map_err(|error| ParameterGolfSingleH100VisualizationError::Write {
            path: metadata.run_root.display().to_string(),
            error,
        })?;
        let mut writer = Self {
            metadata,
            paths,
            geometry: config.geometry.clone(),
            state: ParameterGolfSingleH100VisualizationState {
                run_id: config.run_id.clone(),
                result_classification: RemoteTrainingResultClassification::Active,
                started_at_ms,
                finished_at_ms: None,
                phase: String::from("provisioning"),
                subphase: Some(String::from("trainer_boot")),
                step_in_progress: None,
                microbatch_in_progress: None,
                active_subsystems: vec![String::from("trainer_boot")],
                summary_detail: String::from(
                    "The single-H100 trainer has started and is emitting live visualization bundles.",
                ),
                heartbeat_seq: 0,
                timeline: vec![RemoteTrainingTimelineEntry {
                    observed_at_ms: started_at_ms,
                    phase: String::from("provisioning"),
                    subphase: Some(String::from("trainer_boot")),
                    detail: String::from(
                        "The single-H100 trainer started and created its live visualization mirror.",
                    ),
                }],
                event_series: vec![RemoteTrainingEventSample {
                    observed_at_ms: started_at_ms,
                    severity: RemoteTrainingEventSeverity::Info,
                    event_kind: String::from("trainer_started"),
                    detail: String::from(
                        "The single-H100 trainer created its provider-neutral live visualization bundle.",
                    ),
                }],
                heartbeat_series: Vec::new(),
                gpu_series: Vec::new(),
                step_metrics: Vec::new(),
                validation_checkpoints: Vec::new(),
                initial_validation: None,
                pre_export_final_validation: None,
                final_validation: None,
                final_roundtrip_receipt: None,
                training_report_digest: None,
                used_log_fallback: false,
            },
            last_flush_at_ms: None,
        };
        writer.flush(true)?;
        Ok(Some(writer))
    }

    pub fn record_phase(
        &mut self,
        phase: impl Into<String>,
        subphase: Option<String>,
        detail: impl Into<String>,
        active_subsystems: Vec<String>,
        step_in_progress: Option<u64>,
        microbatch_in_progress: Option<u32>,
        force_flush: bool,
    ) -> Result<(), ParameterGolfSingleH100VisualizationError> {
        let observed_at_ms = unix_time_ms();
        let phase = phase.into();
        let detail = detail.into();
        let timeline_changed = self.state.phase != phase || self.state.subphase != subphase;
        self.state.phase = phase.clone();
        self.state.subphase = subphase.clone();
        self.state.active_subsystems = dedup_strings(active_subsystems);
        self.state.step_in_progress = step_in_progress;
        self.state.microbatch_in_progress = microbatch_in_progress;
        if timeline_changed {
            self.state.timeline.push(RemoteTrainingTimelineEntry {
                observed_at_ms,
                phase,
                subphase,
                detail,
            });
        }
        self.flush(force_flush)
    }

    pub fn record_event(
        &mut self,
        severity: RemoteTrainingEventSeverity,
        event_kind: impl Into<String>,
        detail: impl Into<String>,
    ) {
        self.state.event_series.push(RemoteTrainingEventSample {
            observed_at_ms: unix_time_ms(),
            severity,
            event_kind: event_kind.into(),
            detail: detail.into(),
        });
    }

    pub fn record_step(
        &mut self,
        step_metrics: ParameterGolfSingleH100TrainingStepMetrics,
    ) -> Result<(), ParameterGolfSingleH100VisualizationError> {
        self.state.summary_detail = format!(
            "The single-H100 trainer completed optimizer step {} and retained the full math, runtime, and loss series.",
            step_metrics.global_step
        );
        self.state.step_metrics.push(step_metrics);
        self.flush(false)
    }

    pub fn record_validation_checkpoint(
        &mut self,
        checkpoint: ParameterGolfSingleH100ValidationCheckpoint,
        slot: ValidationSlot,
    ) -> Result<(), ParameterGolfSingleH100VisualizationError> {
        match slot {
            ValidationSlot::Initial => {
                self.state.initial_validation = Some(checkpoint.summary.clone());
            }
            ValidationSlot::Periodic => {
                self.state.validation_checkpoints.push(checkpoint);
            }
            ValidationSlot::PreExportFinal => {
                self.state.pre_export_final_validation = Some(checkpoint.summary.clone());
            }
            ValidationSlot::FinalRoundtrip => {
                self.state.final_validation = Some(checkpoint.summary.clone());
            }
        }
        self.flush(false)
    }

    pub fn record_roundtrip_receipt(
        &mut self,
        receipt: ParameterGolfSingleH100RoundtripReceipt,
    ) -> Result<(), ParameterGolfSingleH100VisualizationError> {
        self.state.final_roundtrip_receipt = Some(receipt);
        self.flush(true)
    }

    pub fn finish_with_report(
        &mut self,
        report: &ParameterGolfSingleH100TrainingReport,
    ) -> Result<
        ParameterGolfSingleH100VisualizationWriteOutcome,
        ParameterGolfSingleH100VisualizationError,
    > {
        self.state.result_classification = map_report_result(report);
        self.state.finished_at_ms = Some(report.finished_at_ms);
        self.state.step_in_progress = None;
        self.state.microbatch_in_progress = None;
        self.state.active_subsystems = vec![String::from("artifact_seal")];
        self.state.phase = String::from("complete");
        self.state.subphase = Some(match report.stop_reason {
            Some(ParameterGolfSingleH100TrainingStopReason::StepBudgetReached) => {
                String::from("step_budget_reached")
            }
            Some(ParameterGolfSingleH100TrainingStopReason::WallclockCapReached) => {
                String::from("wallclock_cap_reached")
            }
            None => String::from("report_written"),
        });
        self.state.summary_detail = report.summary.clone();
        self.state.training_report_digest = Some(report.report_digest.clone());
        self.state.timeline.push(RemoteTrainingTimelineEntry {
            observed_at_ms: report.finished_at_ms,
            phase: self.state.phase.clone(),
            subphase: self.state.subphase.clone(),
            detail: report.summary.clone(),
        });
        self.state.event_series.push(RemoteTrainingEventSample {
            observed_at_ms: report.finished_at_ms,
            severity: RemoteTrainingEventSeverity::Info,
            event_kind: String::from("trainer_report_written"),
            detail: String::from(
                "The single-H100 trainer sealed its authoritative training report.",
            ),
        });
        self.flush(true)?;
        write_visualization_artifacts_from_report(
            self.metadata.clone(),
            report,
            Some(&self.current_bundle()?),
        )
    }

    pub fn current_bundle(
        &self,
    ) -> Result<RemoteTrainingVisualizationBundle, ParameterGolfSingleH100VisualizationError> {
        build_bundle_from_state(&self.metadata, &self.state, self.geometry.clone())
    }

    pub fn flush(&mut self, force: bool) -> Result<(), ParameterGolfSingleH100VisualizationError> {
        let observed_at_ms = unix_time_ms();
        if !force
            && self.last_flush_at_ms.is_some_and(|last_flush| {
                observed_at_ms.saturating_sub(last_flush)
                    < REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS
            })
        {
            return Ok(());
        }
        self.state.heartbeat_seq = self.state.heartbeat_seq.saturating_add(1);
        self.state
            .heartbeat_series
            .push(RemoteTrainingHeartbeatSample {
                observed_at_ms,
                phase: self.state.phase.clone(),
                subphase: self.state.subphase.clone(),
                step_in_progress: self.state.step_in_progress,
                microbatch_in_progress: self.state.microbatch_in_progress,
                active_subsystems: dedup_strings(self.state.active_subsystems.clone()),
                stale_after_ms: PARAMETER_GOLF_SINGLE_H100_LIVE_STALE_AFTER_MS,
            });
        self.state
            .gpu_series
            .extend(collect_local_gpu_samples(observed_at_ms));
        let bundle = build_bundle_from_state(&self.metadata, &self.state, self.geometry.clone())?;
        let run_index = build_run_index(&self.metadata, &bundle)?;
        write_atomically_json(self.paths.bundle_path.as_path(), &bundle)?;
        write_atomically_json(self.paths.run_index_path.as_path(), &run_index)?;
        self.last_flush_at_ms = Some(observed_at_ms);
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValidationSlot {
    Initial,
    Periodic,
    PreExportFinal,
    FinalRoundtrip,
}

pub fn write_visualization_artifacts_from_report(
    metadata: ParameterGolfSingleH100VisualizationMetadata,
    report: &ParameterGolfSingleH100TrainingReport,
    existing_live_bundle: Option<&RemoteTrainingVisualizationBundle>,
) -> Result<
    ParameterGolfSingleH100VisualizationWriteOutcome,
    ParameterGolfSingleH100VisualizationError,
> {
    let paths = metadata.paths();
    fs::create_dir_all(
        paths
            .bundle_path
            .parent()
            .expect("bundle path should have a parent"),
    )
    .map_err(|error| ParameterGolfSingleH100VisualizationError::Write {
        path: metadata.run_root.display().to_string(),
        error,
    })?;
    let state = state_from_report(report, existing_live_bundle);
    let bundle = build_bundle_from_state(&metadata, &state, report.geometry.clone())?;
    let run_index = build_run_index(&metadata, &bundle)?;
    write_atomically_json(paths.bundle_path.as_path(), &bundle)?;
    write_atomically_json(paths.run_index_path.as_path(), &run_index)?;
    Ok(ParameterGolfSingleH100VisualizationWriteOutcome {
        bundle,
        run_index,
        paths,
    })
}

pub fn write_visualization_artifacts_from_log_fallback(
    metadata: ParameterGolfSingleH100VisualizationMetadata,
    log_path: &Path,
    result_classification: RemoteTrainingResultClassification,
) -> Result<
    ParameterGolfSingleH100VisualizationWriteOutcome,
    ParameterGolfSingleH100VisualizationError,
> {
    let paths = metadata.paths();
    fs::create_dir_all(
        paths
            .bundle_path
            .parent()
            .expect("bundle path should have a parent"),
    )
    .map_err(|error| ParameterGolfSingleH100VisualizationError::Write {
        path: metadata.run_root.display().to_string(),
        error,
    })?;
    let log_text = fs::read_to_string(log_path).map_err(|error| {
        ParameterGolfSingleH100VisualizationError::Read {
            path: log_path.display().to_string(),
            error,
        }
    })?;
    let state = state_from_log_fallback(&metadata, log_text.as_str(), result_classification)?;
    let bundle = build_bundle_from_state(
        &metadata,
        &state,
        ParameterGolfBatchGeometry::challenge_single_device_defaults(),
    )?;
    let run_index = build_run_index(&metadata, &bundle)?;
    write_atomically_json(paths.bundle_path.as_path(), &bundle)?;
    write_atomically_json(paths.run_index_path.as_path(), &run_index)?;
    Ok(ParameterGolfSingleH100VisualizationWriteOutcome {
        bundle,
        run_index,
        paths,
    })
}

pub fn read_visualization_bundle(
    bundle_path: &Path,
) -> Result<Option<RemoteTrainingVisualizationBundle>, ParameterGolfSingleH100VisualizationError> {
    if !bundle_path.is_file() {
        return Ok(None);
    }
    let raw = fs::read_to_string(bundle_path).map_err(|error| {
        ParameterGolfSingleH100VisualizationError::Read {
            path: bundle_path.display().to_string(),
            error,
        }
    })?;
    let bundle = serde_json::from_str::<RemoteTrainingVisualizationBundle>(raw.as_str()).map_err(
        |error| ParameterGolfSingleH100VisualizationError::Deserialize {
            path: bundle_path.display().to_string(),
            error,
        },
    )?;
    bundle.validate()?;
    Ok(Some(bundle))
}

pub(crate) fn gradient_clip_observation(
    gradients: &[(String, Vec<f32>)],
    max_norm: f32,
) -> GradientClipObservation {
    if !(max_norm.is_finite() && max_norm > 0.0) {
        return GradientClipObservation {
            gradient_norm_after_clip: finite_l2_norm(gradients),
            clip_applied: false,
            non_finite_count: non_finite_value_count(gradients),
        };
    }
    let non_finite_count = non_finite_value_count(gradients);
    let norm = finite_l2_norm(gradients);
    if norm.is_none_or(|value| value <= max_norm || value <= f32::EPSILON) {
        return GradientClipObservation {
            gradient_norm_after_clip: norm,
            clip_applied: false,
            non_finite_count,
        };
    }
    GradientClipObservation {
        gradient_norm_after_clip: Some(max_norm),
        clip_applied: true,
        non_finite_count,
    }
}

pub(crate) fn state_parameter_norm(
    state: &crate::parameter_golf_single_h100_training::ParameterGolfSingleH100TrainerState,
) -> Option<f32> {
    let mut sum = 0.0_f64;
    let mut saw_value = false;
    for parameter_state in state.parameter_states.values() {
        for value in parameter_state.values() {
            saw_value = true;
            let value = f64::from(*value);
            sum += value * value;
        }
    }
    saw_value.then_some(sum.sqrt() as f32)
}

fn build_bundle_from_state(
    metadata: &ParameterGolfSingleH100VisualizationMetadata,
    state: &ParameterGolfSingleH100VisualizationState,
    geometry: ParameterGolfBatchGeometry,
) -> Result<RemoteTrainingVisualizationBundle, ParameterGolfSingleH100VisualizationError> {
    let validation_by_step = state
        .validation_checkpoints
        .iter()
        .map(|checkpoint| (checkpoint.trigger_step, checkpoint.summary.mean_loss as f32))
        .collect::<BTreeMap<_, _>>();
    let mut elapsed_ms = 0_u64;
    let mut loss_series = Vec::new();
    let mut math_series = Vec::new();
    let mut runtime_series = Vec::new();
    let mut latest_train_loss = None;
    let mut latest_validation_loss = None;
    let mut latest_tokens_per_second = None;
    let mut latest_samples_per_second_milli = None;
    for step in &state.step_metrics {
        elapsed_ms = elapsed_ms.saturating_add(step.observed_wallclock_ms);
        let observed_at_ms = state.started_at_ms.saturating_add(elapsed_ms);
        let validation_loss = validation_by_step
            .get(&step.global_step)
            .copied()
            .or_else(|| {
                (Some(step.global_step) == state.step_metrics.last().map(|row| row.global_step))
                    .then_some(())
                    .and_then(|_| {
                        state
                            .final_validation
                            .as_ref()
                            .map(|summary| summary.mean_loss as f32)
                    })
            });
        latest_train_loss = Some(step.mean_microbatch_loss);
        if let Some(validation_loss) = validation_loss {
            latest_validation_loss = Some(validation_loss);
        }
        latest_tokens_per_second = step.tokens_per_second;
        latest_samples_per_second_milli = step.samples_per_second_milli;
        loss_series.push(RemoteTrainingLossSample {
            global_step: Some(step.global_step),
            elapsed_ms,
            train_loss: Some(step.mean_microbatch_loss),
            ema_loss: None,
            validation_loss,
        });
        math_series.push(RemoteTrainingMathSample {
            observed_at_ms,
            global_step: Some(step.global_step),
            learning_rate: step.effective_learning_rate,
            gradient_norm: step.gradient_norm_after_clip,
            parameter_norm: step.parameter_norm_after_step,
            update_norm: step.update_norm,
            clip_fraction: Some(if step.clip_applied { 1.0 } else { 0.0 }),
            clip_event_count: Some(u32::from(step.clip_applied)),
            loss_scale: None,
            non_finite_count: step.non_finite_gradient_count,
            model_specific_diagnostics: BTreeMap::from([
                (
                    String::from("learning_rate_multiplier"),
                    step.learning_rate_multiplier,
                ),
                (String::from("muon_momentum"), step.muon_momentum),
            ]),
        });
        let data_wait_ms = step
            .phase_timings
            .window_planning_ms
            .saturating_add(step.phase_timings.token_materialization_ms);
        runtime_series.push(RemoteTrainingRuntimeSample {
            observed_at_ms,
            data_wait_ms: nonzero_duration_ms(data_wait_ms),
            forward_ms: nonzero_duration_ms(step.phase_timings.forward_loss_cuda_ms),
            backward_ms: nonzero_duration_ms(step.phase_timings.backward_cuda_ms),
            optimizer_ms: nonzero_duration_ms(step.phase_timings.optimizer_step_ms),
            checkpoint_ms: None,
            evaluation_ms: None,
            tokens_per_second: step.tokens_per_second.or_else(|| {
                throughput_tokens_per_second(
                    geometry.local_train_batch_tokens(),
                    step.observed_wallclock_ms,
                )
            }),
            samples_per_second_milli: step.samples_per_second_milli.or_else(|| {
                throughput_samples_per_second_milli(
                    geometry.local_train_batch_sequences(),
                    step.observed_wallclock_ms,
                )
            }),
        });
    }
    let summary_detail = if state.used_log_fallback {
        String::from(
            "Trainer JSON was absent. This bundle preserves a log-derived fallback series until the authoritative trainer report exists.",
        )
    } else {
        state.summary_detail.clone()
    };
    let latest_checkpoint_ref = state
        .final_roundtrip_receipt
        .as_ref()
        .map(|receipt| receipt.compressed_model_artifact_ref.clone());
    let series_status = if state.step_metrics.is_empty() {
        RemoteTrainingSeriesStatus::Unavailable
    } else if state.used_log_fallback {
        RemoteTrainingSeriesStatus::Partial
    } else {
        RemoteTrainingSeriesStatus::Available
    };
    let series_unavailable_reason = match series_status {
        RemoteTrainingSeriesStatus::Available => None,
        RemoteTrainingSeriesStatus::Partial => Some(String::from(
            "trainer report was absent and the bundle preserved a log-derived fallback loss series",
        )),
        RemoteTrainingSeriesStatus::Unavailable => Some(String::from(
            "no single-H100 step series was retained for this run yet",
        )),
    };
    let bundle = record_remote_training_visualization_bundle(RemoteTrainingVisualizationBundle {
        schema_version: String::new(),
        bundle_id: format!("{}-remote-training-v1", state.run_id),
        provider: metadata.provider,
        profile_id: metadata.profile_id.clone(),
        lane_id: metadata.lane_id.clone(),
        run_id: state.run_id.clone(),
        repo_revision: metadata.repo_revision.clone(),
        result_classification: state.result_classification,
        refresh_contract: RemoteTrainingRefreshContract {
            target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
            emission_mode: if state.result_classification
                == RemoteTrainingResultClassification::Active
            {
                RemoteTrainingEmissionMode::AppendOnlySnapshots
            } else if state.used_log_fallback && state.heartbeat_series.is_empty() {
                RemoteTrainingEmissionMode::PostRunOnly
            } else {
                RemoteTrainingEmissionMode::AppendOnlySnapshots
            },
            last_heartbeat_at_ms: state
                .heartbeat_series
                .last()
                .map(|sample| sample.observed_at_ms),
            heartbeat_seq: state.heartbeat_seq.max(state.heartbeat_series.len() as u64),
        },
        series_status,
        series_unavailable_reason,
        timeline: if state.timeline.is_empty() {
            vec![RemoteTrainingTimelineEntry {
                observed_at_ms: state.started_at_ms,
                phase: state.phase.clone(),
                subphase: state.subphase.clone(),
                detail: state.summary_detail.clone(),
            }]
        } else {
            state.timeline.clone()
        },
        summary: RemoteTrainingVisualizationSummary {
            total_steps_completed: state.step_metrics.len() as u64,
            latest_global_step: state.step_metrics.last().map(|step| step.global_step),
            latest_train_loss,
            latest_ema_loss: None,
            latest_validation_loss,
            latest_tokens_per_second,
            latest_samples_per_second_milli,
            accumulated_cost_microusd: None,
            latest_checkpoint_ref,
            detail: summary_detail,
        },
        heartbeat_series: state.heartbeat_series.clone(),
        loss_series,
        math_series,
        runtime_series,
        gpu_series: state.gpu_series.clone(),
        distributed_series: Vec::new(),
        event_series: state.event_series.clone(),
        source_artifacts: build_source_artifacts(metadata, state),
        bundle_digest: String::new(),
    })?;
    Ok(bundle)
}

fn build_run_index(
    metadata: &ParameterGolfSingleH100VisualizationMetadata,
    bundle: &RemoteTrainingVisualizationBundle,
) -> Result<RemoteTrainingRunIndex, ParameterGolfSingleH100VisualizationError> {
    build_remote_training_run_index(RemoteTrainingRunIndex {
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
            summary_label: format!(
                "{} {} single-H100",
                provider_label(bundle.provider),
                metadata.profile_id
            ),
            detail: bundle.summary.detail.clone(),
        }],
        detail: String::from(
            "This run index enumerates the live or finalized single-H100 Parameter Golf visualization bundle for the app mirror.",
        ),
        index_digest: String::new(),
    })
    .map_err(Into::into)
}

fn build_source_artifacts(
    metadata: &ParameterGolfSingleH100VisualizationMetadata,
    state: &ParameterGolfSingleH100VisualizationState,
) -> Vec<RemoteTrainingSourceArtifact> {
    let mut artifacts = vec![RemoteTrainingSourceArtifact {
        artifact_role: String::from("live_bundle"),
        artifact_uri: metadata.bundle_relative_path(),
        artifact_digest: None,
        source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
        authoritative: true,
        source_receipt_ids: vec![String::from(
            "parameter_golf_single_h100.remote_training_live_bundle.v1",
        )],
        detail: String::from(
            "The provider-neutral local mirror is the app-facing source for this single-H100 lane.",
        ),
    }];
    if let Some(report_digest) = &state.training_report_digest {
        artifacts.push(RemoteTrainingSourceArtifact {
            artifact_role: String::from("trainer_report"),
            artifact_uri: metadata.report_relative_path(),
            artifact_digest: Some(report_digest.clone()),
            source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
            authoritative: true,
            source_receipt_ids: vec![String::from(
                "parameter_golf_single_h100_training_report.v1",
            )],
            detail: String::from(
                "The trainer JSON is authoritative for the full retained step, math, and runtime series.",
            ),
        });
    } else {
        artifacts.push(RemoteTrainingSourceArtifact {
            artifact_role: String::from("training_log_fallback"),
            artifact_uri: String::from("parameter_golf_single_h100_train.log"),
            artifact_digest: None,
            source_kind: RemoteTrainingArtifactSourceKind::LogDerivedFallback,
            authoritative: false,
            source_receipt_ids: Vec::new(),
            detail: String::from(
                "The trainer JSON was absent, so the visualization bundle fell back to log-derived progress samples.",
            ),
        });
    }
    artifacts
}

fn state_from_report(
    report: &ParameterGolfSingleH100TrainingReport,
    existing_live_bundle: Option<&RemoteTrainingVisualizationBundle>,
) -> ParameterGolfSingleH100VisualizationState {
    let (mut timeline, mut event_series, mut heartbeat_series, gpu_series, mut heartbeat_seq) =
        existing_live_bundle.map_or_else(
            || {
                let timeline = vec![
                    RemoteTrainingTimelineEntry {
                        observed_at_ms: report.started_at_ms,
                        phase: String::from("training"),
                        subphase: Some(String::from("optimizer_step")),
                        detail: String::from(
                            "The single-H100 trainer entered measured optimizer steps.",
                        ),
                    },
                    RemoteTrainingTimelineEntry {
                        observed_at_ms: report.finished_at_ms,
                        phase: String::from("complete"),
                        subphase: Some(String::from("report_written")),
                        detail: report.summary.clone(),
                    },
                ];
                let heartbeat_series = synthetic_report_heartbeats(report);
                (
                    timeline,
                    vec![RemoteTrainingEventSample {
                        observed_at_ms: report.finished_at_ms,
                        severity: RemoteTrainingEventSeverity::Info,
                        event_kind: String::from("trainer_report_written"),
                        detail: String::from(
                            "The single-H100 trainer sealed its authoritative training report.",
                        ),
                    }],
                    heartbeat_series.clone(),
                    Vec::new(),
                    heartbeat_series.len() as u64,
                )
            },
            |bundle| {
                (
                    bundle.timeline.clone(),
                    bundle.event_series.clone(),
                    bundle.heartbeat_series.clone(),
                    bundle.gpu_series.clone(),
                    bundle.refresh_contract.heartbeat_seq,
                )
            },
        );
    if timeline.last().map(|entry| entry.phase.as_str()) != Some("complete") {
        timeline.push(RemoteTrainingTimelineEntry {
            observed_at_ms: report.finished_at_ms,
            phase: String::from("complete"),
            subphase: Some(String::from("report_written")),
            detail: report.summary.clone(),
        });
    }
    if !event_series
        .iter()
        .any(|event| event.event_kind == "trainer_report_written")
    {
        event_series.push(RemoteTrainingEventSample {
            observed_at_ms: report.finished_at_ms,
            severity: RemoteTrainingEventSeverity::Info,
            event_kind: String::from("trainer_report_written"),
            detail: String::from(
                "The single-H100 trainer sealed its authoritative training report.",
            ),
        });
    }
    if heartbeat_series
        .last()
        .map(|sample| sample.observed_at_ms < report.finished_at_ms)
        .unwrap_or(true)
    {
        heartbeat_series.push(RemoteTrainingHeartbeatSample {
            observed_at_ms: report.finished_at_ms,
            phase: String::from("complete"),
            subphase: Some(String::from("report_written")),
            step_in_progress: None,
            microbatch_in_progress: None,
            active_subsystems: vec![String::from("artifact_seal")],
            stale_after_ms: PARAMETER_GOLF_SINGLE_H100_LIVE_STALE_AFTER_MS,
        });
    }
    heartbeat_seq = heartbeat_seq.max(heartbeat_series.len() as u64);
    ParameterGolfSingleH100VisualizationState {
        run_id: report.run_id.clone(),
        result_classification: map_report_result(report),
        started_at_ms: report.started_at_ms,
        finished_at_ms: Some(report.finished_at_ms),
        phase: String::from("complete"),
        subphase: Some(String::from("report_written")),
        step_in_progress: None,
        microbatch_in_progress: None,
        active_subsystems: vec![String::from("artifact_seal")],
        summary_detail: report.summary.clone(),
        heartbeat_seq,
        timeline,
        event_series,
        heartbeat_series,
        gpu_series,
        step_metrics: report.step_metrics.clone(),
        validation_checkpoints: report.validation_checkpoints.clone(),
        initial_validation: report.initial_validation.clone(),
        pre_export_final_validation: report.pre_export_final_validation.clone(),
        final_validation: report.final_validation.clone(),
        final_roundtrip_receipt: report.final_roundtrip_receipt.clone(),
        training_report_digest: Some(report.report_digest.clone()),
        used_log_fallback: false,
    }
}

fn state_from_log_fallback(
    metadata: &ParameterGolfSingleH100VisualizationMetadata,
    log_text: &str,
    result_classification: RemoteTrainingResultClassification,
) -> Result<ParameterGolfSingleH100VisualizationState, ParameterGolfSingleH100VisualizationError> {
    let mut steps = BTreeMap::<u64, LogFallbackStep>::new();
    let mut validation_checkpoints = Vec::new();
    let mut current_step_in_progress = None;
    let mut current_microbatch_in_progress = None;
    let mut grad_accum_steps = None;
    for line in log_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(step_value) = token_u64(line, "train_step_start", "step") {
            current_step_in_progress = Some(step_value);
            current_microbatch_in_progress = Some(0);
            grad_accum_steps = token_u32(line, "train_step_start", "grad_accum_steps");
            continue;
        }
        if let Some(step_value) = token_u64(line, "micro_step_complete", "step") {
            let entry = steps.entry(step_value).or_default();
            entry.global_step = step_value;
            entry.mean_microbatch_loss = token_f32(line, "micro_step_complete", "train_loss");
            entry.forward_ms = token_u64(line, "micro_step_complete", "forward_ms");
            entry.backward_ms = token_u64(line, "micro_step_complete", "backward_ms");
            entry.host_materialization_ms =
                token_u64(line, "micro_step_complete", "host_materialization_ms");
            current_step_in_progress = Some(step_value);
            current_microbatch_in_progress = token_u32(line, "micro_step_complete", "micro_step");
            continue;
        }
        if let Some(step_value) = token_u64(line, "train_step_complete", "step") {
            let entry = steps.entry(step_value).or_default();
            entry.global_step = step_value;
            entry.mean_microbatch_loss =
                token_f32(line, "train_step_complete", "mean_microbatch_loss");
            entry.learning_rate_multiplier = token_f32(line, "train_step_complete", "lr_mult");
            entry.muon_momentum = token_f32(line, "train_step_complete", "muon_momentum");
            entry.optimizer_step_ms = token_u64(line, "train_step_complete", "optimizer_step_ms");
            current_step_in_progress = None;
            current_microbatch_in_progress = None;
            continue;
        }
        if line.starts_with("validation_complete ") {
            let stage_label = token_string(line, "validation_complete", "stage")
                .unwrap_or_else(|| String::from("validation"));
            let trigger_step = token_u64(line, "validation_complete", "trigger_step")
                .unwrap_or_else(|| steps.keys().next_back().copied().unwrap_or(0));
            let observed_validation_ms =
                token_u64(line, "validation_complete", "elapsed_ms").unwrap_or(0);
            if let (Some(mean_loss), Some(bits_per_byte)) = (
                token_f64(line, "validation_complete", "mean_loss"),
                token_f64(line, "validation_complete", "val_bpb"),
            ) {
                validation_checkpoints.push(ParameterGolfSingleH100ValidationCheckpoint {
                    stage_label,
                    trigger_step,
                    observed_training_time_ms: 0,
                    observed_validation_ms,
                    summary: ParameterGolfSingleH100ValidationSummary {
                        eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
                        evaluated_sequence_count: 0,
                        evaluated_token_count: token_u64(
                            line,
                            "validation_complete",
                            "evaluated_tokens",
                        )
                        .unwrap_or(0),
                        evaluated_byte_count: token_u64(
                            line,
                            "validation_complete",
                            "evaluated_bytes",
                        )
                        .unwrap_or(0),
                        mean_loss,
                        bits_per_byte,
                        runtime_receipt: None,
                        score_first_ttt_receipt: None,
                    },
                });
            }
        }
    }
    if steps.is_empty() {
        return Err(ParameterGolfSingleH100VisualizationError::MissingLogFallbackSeries);
    }
    let mut step_metrics = Vec::new();
    for step in steps.into_values() {
        let observed_wallclock_ms = step
            .forward_ms
            .unwrap_or(0)
            .saturating_add(step.backward_ms.unwrap_or(0))
            .saturating_add(step.host_materialization_ms.unwrap_or(0))
            .saturating_add(step.optimizer_step_ms.unwrap_or(0));
        step_metrics.push(ParameterGolfSingleH100TrainingStepMetrics {
            global_step: step.global_step,
            train_window_ids: Vec::new(),
            mean_microbatch_loss: step.mean_microbatch_loss.unwrap_or(0.0),
            learning_rate_multiplier: step.learning_rate_multiplier.unwrap_or(1.0),
            muon_momentum: step.muon_momentum.unwrap_or(0.0),
            observed_wallclock_ms: observed_wallclock_ms.max(1),
            phase_timings: crate::ParameterGolfSingleH100PhaseTimings {
                forward_loss_cuda_ms: step.forward_ms.unwrap_or(0),
                backward_cuda_ms: step.backward_ms.unwrap_or(0),
                host_gradient_materialization_ms: step.host_materialization_ms.unwrap_or(0),
                optimizer_step_ms: step.optimizer_step_ms.unwrap_or(0),
                ..Default::default()
            },
            effective_learning_rate: None,
            gradient_norm_after_clip: None,
            parameter_norm_after_step: None,
            update_norm: None,
            clip_applied: false,
            non_finite_gradient_count: 0,
            tokens_per_second: throughput_tokens_per_second(
                ParameterGolfBatchGeometry::challenge_single_device_defaults()
                    .local_train_batch_tokens(),
                observed_wallclock_ms.max(1),
            ),
            samples_per_second_milli: throughput_samples_per_second_milli(
                ParameterGolfBatchGeometry::challenge_single_device_defaults()
                    .local_train_batch_sequences(),
                observed_wallclock_ms.max(1),
            ),
            runtime_receipt: None,
        });
    }
    let total_elapsed_ms = step_metrics
        .iter()
        .map(|step| step.observed_wallclock_ms)
        .sum::<u64>()
        .max(1);
    let finished_at_ms = unix_time_ms();
    let started_at_ms = finished_at_ms.saturating_sub(total_elapsed_ms);
    let heartbeat_series = synthetic_log_fallback_heartbeats(
        started_at_ms,
        step_metrics.as_slice(),
        current_step_in_progress,
        current_microbatch_in_progress,
        grad_accum_steps.unwrap_or(0),
    );
    Ok(ParameterGolfSingleH100VisualizationState {
        run_id: metadata
            .report_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(String::from)
            .unwrap_or_else(|| String::from("parameter-golf-single-h100")),
        result_classification,
        started_at_ms,
        finished_at_ms: (result_classification != RemoteTrainingResultClassification::Active)
            .then_some(finished_at_ms),
        phase: if result_classification == RemoteTrainingResultClassification::Active {
            String::from("training")
        } else {
            String::from("complete")
        },
        subphase: if result_classification == RemoteTrainingResultClassification::Active {
            Some(String::from("optimizer_step"))
        } else {
            Some(String::from("log_fallback"))
        },
        step_in_progress: current_step_in_progress,
        microbatch_in_progress: current_microbatch_in_progress,
        active_subsystems: if result_classification == RemoteTrainingResultClassification::Active {
            vec![String::from("log_replay"), String::from("training")]
        } else {
            vec![String::from("log_replay"), String::from("artifact_seal")]
        },
        summary_detail: String::from(
            "The trainer report was absent. The bundle is preserving a partial log-derived fallback series.",
        ),
        heartbeat_seq: heartbeat_series.len() as u64,
        timeline: vec![
            RemoteTrainingTimelineEntry {
                observed_at_ms: started_at_ms,
                phase: String::from("training"),
                subphase: Some(String::from("log_fallback")),
                detail: String::from(
                    "The visualization bundle reconstructed a partial series from the retained trainer log because the trainer JSON was absent.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: finished_at_ms,
                phase: if result_classification == RemoteTrainingResultClassification::Active {
                    String::from("training")
                } else {
                    String::from("complete")
                },
                subphase: Some(String::from("log_fallback")),
                detail: String::from(
                    "The log-derived fallback series was sealed with explicit partial-series posture.",
                ),
            },
        ],
        event_series: vec![RemoteTrainingEventSample {
            observed_at_ms: finished_at_ms,
            severity: RemoteTrainingEventSeverity::Warning,
            event_kind: String::from("log_fallback_series"),
            detail: String::from(
                "The trainer JSON was absent, so the bundle retained a partial log-derived fallback series.",
            ),
        }],
        heartbeat_series,
        gpu_series: Vec::new(),
        step_metrics,
        validation_checkpoints,
        initial_validation: None,
        pre_export_final_validation: None,
        final_validation: None,
        final_roundtrip_receipt: None,
        training_report_digest: None,
        used_log_fallback: true,
    })
}

fn synthetic_report_heartbeats(
    report: &ParameterGolfSingleH100TrainingReport,
) -> Vec<RemoteTrainingHeartbeatSample> {
    let mut elapsed_ms = 0_u64;
    report
        .step_metrics
        .iter()
        .map(|step| {
            elapsed_ms = elapsed_ms.saturating_add(step.observed_wallclock_ms);
            RemoteTrainingHeartbeatSample {
                observed_at_ms: report.started_at_ms.saturating_add(elapsed_ms),
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                step_in_progress: Some(step.global_step),
                microbatch_in_progress: Some(0),
                active_subsystems: vec![
                    String::from("optimizer"),
                    String::from("math_retention"),
                    String::from("runtime_retention"),
                ],
                stale_after_ms: PARAMETER_GOLF_SINGLE_H100_LIVE_STALE_AFTER_MS,
            }
        })
        .collect()
}

fn synthetic_log_fallback_heartbeats(
    started_at_ms: u64,
    step_metrics: &[ParameterGolfSingleH100TrainingStepMetrics],
    step_in_progress: Option<u64>,
    microbatch_in_progress: Option<u32>,
    grad_accum_steps: u32,
) -> Vec<RemoteTrainingHeartbeatSample> {
    let mut elapsed_ms = 0_u64;
    let mut heartbeats = step_metrics
        .iter()
        .map(|step| {
            elapsed_ms = elapsed_ms.saturating_add(step.observed_wallclock_ms);
            RemoteTrainingHeartbeatSample {
                observed_at_ms: started_at_ms.saturating_add(elapsed_ms),
                phase: String::from("training"),
                subphase: Some(String::from("log_fallback")),
                step_in_progress: Some(step.global_step),
                microbatch_in_progress: Some(grad_accum_steps.max(1)),
                active_subsystems: vec![String::from("log_replay"), String::from("optimizer")],
                stale_after_ms: PARAMETER_GOLF_SINGLE_H100_LIVE_STALE_AFTER_MS,
            }
        })
        .collect::<Vec<_>>();
    if let Some(step_in_progress) = step_in_progress {
        heartbeats.push(RemoteTrainingHeartbeatSample {
            observed_at_ms: unix_time_ms(),
            phase: String::from("training"),
            subphase: Some(String::from("log_fallback")),
            step_in_progress: Some(step_in_progress),
            microbatch_in_progress,
            active_subsystems: vec![String::from("log_replay"), String::from("training")],
            stale_after_ms: PARAMETER_GOLF_SINGLE_H100_LIVE_STALE_AFTER_MS,
        });
    }
    heartbeats
}

fn parse_provider(
    value: &str,
) -> Result<RemoteTrainingProvider, ParameterGolfSingleH100VisualizationError> {
    match value {
        "google_cloud" => Ok(RemoteTrainingProvider::GoogleCloud),
        "run_pod" | "runpod" => Ok(RemoteTrainingProvider::RunPod),
        "local_hybrid" | "localhybrid" => Ok(RemoteTrainingProvider::LocalHybrid),
        other => Err(
            ParameterGolfSingleH100VisualizationError::UnsupportedProvider {
                value: other.to_string(),
            },
        ),
    }
}

fn required_env(name: &str) -> Result<String, ParameterGolfSingleH100VisualizationError> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(
            || ParameterGolfSingleH100VisualizationError::MissingMetadata {
                field: name.to_string(),
            },
        )
}

fn write_atomically_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), ParameterGolfSingleH100VisualizationError> {
    let encoded = serde_json::to_vec_pretty(value).map_err(|error| {
        ParameterGolfSingleH100VisualizationError::Serialize {
            message: error.to_string(),
        }
    })?;
    let temporary_path = path.with_extension("tmp");
    fs::write(&temporary_path, encoded).map_err(|error| {
        ParameterGolfSingleH100VisualizationError::Write {
            path: temporary_path.display().to_string(),
            error,
        }
    })?;
    fs::rename(&temporary_path, path).map_err(|error| {
        ParameterGolfSingleH100VisualizationError::Write {
            path: path.display().to_string(),
            error,
        }
    })
}

fn relative_path_string(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|relative| relative.display().to_string())
}

fn provider_label(provider: RemoteTrainingProvider) -> &'static str {
    match provider {
        RemoteTrainingProvider::GoogleCloud => "google",
        RemoteTrainingProvider::RunPod => "runpod",
        RemoteTrainingProvider::LocalHybrid => "local_hybrid",
    }
}

fn map_report_result(
    report: &ParameterGolfSingleH100TrainingReport,
) -> RemoteTrainingResultClassification {
    match report.disposition {
        crate::ParameterGolfSingleH100TrainingDisposition::TrainingExecuted => {
            RemoteTrainingResultClassification::CompletedSuccess
        }
        crate::ParameterGolfSingleH100TrainingDisposition::RefusedMachineContract
        | crate::ParameterGolfSingleH100TrainingDisposition::RefusedCudaBlockers
        | crate::ParameterGolfSingleH100TrainingDisposition::RefusedWallclockProjection => {
            RemoteTrainingResultClassification::Refused
        }
    }
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
                        trimmed.parse::<u16>().ok()
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

pub(crate) fn finite_l2_norm(gradients: &[(String, Vec<f32>)]) -> Option<f32> {
    let mut sum = 0.0_f64;
    let mut saw_value = false;
    for (_, values) in gradients {
        for value in values {
            if !value.is_finite() {
                return None;
            }
            let value = f64::from(*value);
            sum += value * value;
            saw_value = true;
        }
    }
    saw_value.then_some(sum.sqrt() as f32)
}

pub(crate) fn non_finite_value_count(gradients: &[(String, Vec<f32>)]) -> u32 {
    gradients
        .iter()
        .flat_map(|(_, values)| values.iter())
        .filter(|value| !value.is_finite())
        .count() as u32
}

pub(crate) fn throughput_tokens_per_second(
    batch_tokens: usize,
    observed_wallclock_ms: u64,
) -> Option<u64> {
    (observed_wallclock_ms > 0).then_some(
        ((batch_tokens as u128).saturating_mul(1000) / observed_wallclock_ms as u128) as u64,
    )
}

pub(crate) fn throughput_samples_per_second_milli(
    batch_sequences: usize,
    observed_wallclock_ms: u64,
) -> Option<u32> {
    (observed_wallclock_ms > 0).then_some(
        ((batch_sequences as u128).saturating_mul(1_000_000) / observed_wallclock_ms as u128)
            as u32,
    )
}

fn token_u64(line: &str, prefix: &str, key: &str) -> Option<u64> {
    token_value(line, prefix, key)?
        .split('/')
        .next()?
        .parse::<u64>()
        .ok()
}

fn token_u32(line: &str, prefix: &str, key: &str) -> Option<u32> {
    token_value(line, prefix, key)?
        .split('/')
        .next()?
        .parse::<u32>()
        .ok()
}

fn token_f32(line: &str, prefix: &str, key: &str) -> Option<f32> {
    token_value(line, prefix, key)?.parse::<f32>().ok()
}

fn token_f64(line: &str, prefix: &str, key: &str) -> Option<f64> {
    token_value(line, prefix, key)?.parse::<f64>().ok()
}

fn token_string(line: &str, prefix: &str, key: &str) -> Option<String> {
    token_value(line, prefix, key).map(String::from)
}

fn token_value<'a>(line: &'a str, prefix: &str, key: &str) -> Option<&'a str> {
    if !line.starts_with(prefix) {
        return None;
    }
    let target = format!("{key}=");
    line.split_whitespace()
        .find_map(|token| token.strip_prefix(target.as_str()))
}

fn nonzero_duration_ms(value: u64) -> Option<u64> {
    (value > 0).then_some(value)
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ParameterGolfSingleH100PhaseTimings, ParameterGolfSingleH100PrecisionReceipt,
        ParameterGolfSingleH100TrainingDisposition,
    };
    use psionic_core::{DType, Device, DeviceKind};
    use psionic_data::{DatasetKey, TokenizerDigest, TokenizerFamily};
    use psionic_runtime::{
        DeliveredExecutionContext, DeviceDescriptor, HealthStatus, RuntimeHealth,
    };

    fn sample_report() -> ParameterGolfSingleH100TrainingReport {
        ParameterGolfSingleH100TrainingReport {
            schema_version: 2,
            scope_window: String::from("parameter_golf_single_h100_training_v2"),
            run_id: String::from("parameter-golf-run"),
            dataset_root: PathBuf::from("/tmp/dataset"),
            tokenizer_path: PathBuf::from("/tmp/tokenizer.model"),
            dataset_key: DatasetKey::new("dataset://parameter-golf/fineweb-sp1024", "2026.03.18"),
            variant: String::from("sp1024"),
            tokenizer_digest: TokenizerDigest::new(TokenizerFamily::SentencePiece, "digest", 1024),
            dataset_manifest_digest: String::from("dataset-digest"),
            train_shard_count: 1,
            validation_shard_count: 1,
            train_token_count: 1_048_576,
            validation_token_count: 524_288,
            geometry: ParameterGolfBatchGeometry::challenge_single_device_defaults(),
            hyperparameters: crate::ParameterGolfTrainingHyperparameters::baseline_defaults(),
            max_steps: 2,
            warmup_steps: 0,
            completed_warmup_steps: 0,
            validation_loss_every: 1,
            train_log_every: 1,
            final_validation_mode: crate::ParameterGolfSingleH100ValidationMode::Both,
            validation_eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
            validation_batch_sequences: 64,
            matrix_execution_mode: crate::ParameterGolfMatrixExecutionMode::DirectBanked,
            score_first_ttt: None,
            ema: None,
            swa: None,
            final_model_surface: crate::ParameterGolfFinalModelSurface::Raw,
            final_artifact_config: crate::ParameterGolfFinalArtifactConfig::default(),
            executed_steps: 2,
            stop_reason: Some(ParameterGolfSingleH100TrainingStopReason::StepBudgetReached),
            delivered_execution: DeliveredExecutionContext::new("cuda", None, Vec::new()),
            machine_thresholds: crate::ParameterGolfSingleH100ChallengeThresholds::challenge_h100(),
            observed_cuda_health: RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("ready"),
            },
            cuda_discovery_error: None,
            observed_cuda_devices: vec![DeviceDescriptor {
                backend: String::from("cuda"),
                device: Device::new(DeviceKind::Cuda, 0, Some(String::from("cuda:0"))),
                device_name: Some(String::from("NVIDIA H100 80GB HBM3")),
                supported_dtypes: vec![DType::BF16, DType::F32],
                supported_quantization: Vec::new(),
                memory_capacity_bytes: Some(80 * 1024 * 1024 * 1024),
                unified_memory: Some(false),
                feature_flags: Vec::new(),
                amd_metadata: None,
                nvidia_metadata: None,
            }],
            matching_h100_device_count: 1,
            machine_contract_satisfied: true,
            model_variant: crate::ParameterGolfSingleH100ModelVariant::BaselineSp1024_9x512,
            machine_profile: crate::ParameterGolfSingleDeviceMachineProfile::SingleH100,
            model_config: psionic_models::ParameterGolfConfig::baseline_sp1024_9x512(),
            baseline_model_id: String::from("parameter-golf"),
            baseline_model_revision: String::from("rev"),
            baseline_model_descriptor_digest: String::from("model-digest"),
            optimizer_plan_digest: String::from("optimizer-digest"),
            precision_receipt: ParameterGolfSingleH100PrecisionReceipt {
                graph_parameter_upload_precision: crate::TrainingPrecisionMode::Bf16,
                graph_execution_precision: crate::TrainingPrecisionMode::Fp32,
                retained_activation_precision: crate::TrainingPrecisionMode::Fp32,
                group_receipts: Vec::new(),
                notes: Vec::new(),
            },
            cuda_training_capability_report_digest: String::from("capability-digest"),
            challenge_kernel_blockers: Vec::new(),
            validation_checkpoints: vec![ParameterGolfSingleH100ValidationCheckpoint {
                stage_label: String::from("periodic_validation_step_1"),
                trigger_step: 1,
                observed_training_time_ms: 1050,
                observed_validation_ms: 210,
                summary: ParameterGolfSingleH100ValidationSummary {
                    eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
                    evaluated_sequence_count: 1,
                    evaluated_token_count: 1024,
                    evaluated_byte_count: 980,
                    mean_loss: 4.2,
                    bits_per_byte: 5.9,
                    runtime_receipt: None,
                    score_first_ttt_receipt: None,
                },
            }],
            initial_validation: None,
            pre_export_final_validation: Some(ParameterGolfSingleH100ValidationSummary {
                eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
                evaluated_sequence_count: 1,
                evaluated_token_count: 1024,
                evaluated_byte_count: 980,
                mean_loss: 4.1,
                bits_per_byte: 5.8,
                runtime_receipt: None,
                score_first_ttt_receipt: None,
            }),
            final_validation: Some(ParameterGolfSingleH100ValidationSummary {
                eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
                evaluated_sequence_count: 1,
                evaluated_token_count: 1024,
                evaluated_byte_count: 980,
                mean_loss: 4.0,
                bits_per_byte: 5.7,
                runtime_receipt: None,
                score_first_ttt_receipt: None,
            }),
            warmup_observed_ms: 0,
            observed_training_time_ms: 2_100,
            pre_export_final_validation_observed_ms: Some(200),
            final_validation_observed_ms: Some(190),
            final_swa_receipt: None,
            final_roundtrip_receipt: Some(ParameterGolfSingleH100RoundtripReceipt {
                final_model_surface: crate::ParameterGolfFinalModelSurface::Raw,
                ema: None,
                swa: None,
                artifact_config: crate::ParameterGolfFinalArtifactConfig::default(),
                metric_source: String::from("int8_zlib_roundtrip"),
                validation: ParameterGolfSingleH100ValidationSummary {
                    eval_mode: ParameterGolfValidationEvalMode::NonOverlapping,
                    evaluated_sequence_count: 1,
                    evaluated_token_count: 1024,
                    evaluated_byte_count: 980,
                    mean_loss: 4.0,
                    bits_per_byte: 5.7,
                    runtime_receipt: None,
                    score_first_ttt_receipt: None,
                },
                pre_ttt_validation: None,
                observed_eval_ms: 190,
                compressed_model_bytes: 78_000,
                compressed_model_artifact_ref: String::from("artifact://compressed-model"),
                compressed_model_artifact_digest: String::from("artifact-digest"),
            }),
            compressed_model_bytes: Some(78_000),
            compressed_model_artifact_ref: Some(String::from("artifact://compressed-model")),
            compressed_model_artifact_digest: Some(String::from("artifact-digest")),
            compressed_model_artifact_path: Some(String::from(
                "/tmp/parameter_golf_run.final_model.st",
            )),
            step_metrics: vec![
                ParameterGolfSingleH100TrainingStepMetrics {
                    global_step: 1,
                    train_window_ids: vec![String::from("window-1")],
                    mean_microbatch_loss: 4.5,
                    learning_rate_multiplier: 1.0,
                    muon_momentum: 0.9,
                    observed_wallclock_ms: 1_000,
                    phase_timings: ParameterGolfSingleH100PhaseTimings {
                        forward_loss_cuda_ms: 320,
                        backward_cuda_ms: 360,
                        host_gradient_materialization_ms: 120,
                        optimizer_step_ms: 100,
                        ..Default::default()
                    },
                    effective_learning_rate: Some(0.6),
                    gradient_norm_after_clip: Some(1.2),
                    parameter_norm_after_step: Some(73.0),
                    update_norm: None,
                    clip_applied: false,
                    non_finite_gradient_count: 0,
                    tokens_per_second: Some(140_000),
                    samples_per_second_milli: Some(136),
                    runtime_receipt: None,
                },
                ParameterGolfSingleH100TrainingStepMetrics {
                    global_step: 2,
                    train_window_ids: vec![String::from("window-2")],
                    mean_microbatch_loss: 4.2,
                    learning_rate_multiplier: 0.95,
                    muon_momentum: 0.91,
                    observed_wallclock_ms: 1_100,
                    phase_timings: ParameterGolfSingleH100PhaseTimings {
                        forward_loss_cuda_ms: 315,
                        backward_cuda_ms: 350,
                        host_gradient_materialization_ms: 118,
                        optimizer_step_ms: 102,
                        ..Default::default()
                    },
                    effective_learning_rate: Some(0.57),
                    gradient_norm_after_clip: Some(1.1),
                    parameter_norm_after_step: Some(73.2),
                    update_norm: None,
                    clip_applied: false,
                    non_finite_gradient_count: 0,
                    tokens_per_second: Some(142_000),
                    samples_per_second_milli: Some(138),
                    runtime_receipt: None,
                },
            ],
            aggregate_phase_timings: Some(Default::default()),
            final_training_cursor: None,
            started_at_ms: 1_742_846_400_000,
            finished_at_ms: 1_742_846_402_200,
            observed_wallclock_ms: 2_200,
            disposition: ParameterGolfSingleH100TrainingDisposition::TrainingExecuted,
            refusal: None,
            claim_boundary: String::from("bounded"),
            summary: String::from("completed"),
            report_digest: String::from("report-digest"),
        }
    }

    #[test]
    fn report_materialization_preserves_full_step_series() {
        let report = sample_report();
        let metadata = ParameterGolfSingleH100VisualizationMetadata::new(
            RemoteTrainingProvider::RunPod,
            "runpod_h100_single_gpu",
            "parameter_golf.runpod_single_h100",
            "main@test",
            "/tmp/runpod-run",
            "/tmp/runpod-run/parameter_golf_single_h100_training.json",
        );

        let outcome =
            write_visualization_artifacts_from_report(metadata, &report, None).expect("bundle");

        assert_eq!(outcome.bundle.loss_series.len(), 2);
        assert_eq!(outcome.bundle.math_series.len(), 2);
        assert_eq!(outcome.bundle.runtime_series.len(), 2);
        assert_eq!(
            outcome.bundle.series_status,
            RemoteTrainingSeriesStatus::Available
        );
        assert!(outcome
            .bundle
            .source_artifacts
            .iter()
            .any(|artifact| artifact.artifact_role == "trainer_report"));
    }

    #[test]
    fn log_fallback_is_explicitly_partial() {
        let metadata = ParameterGolfSingleH100VisualizationMetadata::new(
            RemoteTrainingProvider::RunPod,
            "runpod_h100_single_gpu",
            "parameter_golf.runpod_single_h100",
            "main@test",
            "/tmp/runpod-run",
            "/tmp/runpod-run/parameter_golf_single_h100_training.json",
        );
        let log = "\
train_step_start step=1/4 grad_accum_steps=8\n\
micro_step_complete step=1/4 micro_step=8/8 window_id=window-1 train_loss=4.50000000 forward_ms=320 backward_ms=360 host_materialization_ms=120 retained_binding_f32=0 gradient_f32=0\n\
train_step_complete step=1 mean_microbatch_loss=4.50000000 lr_mult=1.00000000 muon_momentum=0.90000000 host_materialization_ms=120 optimizer_step_ms=100\n";

        let state =
            state_from_log_fallback(&metadata, log, RemoteTrainingResultClassification::Active)
                .expect("log fallback");
        let bundle = build_bundle_from_state(
            &metadata,
            &state,
            ParameterGolfBatchGeometry::challenge_single_device_defaults(),
        )
        .expect("bundle");

        assert_eq!(bundle.series_status, RemoteTrainingSeriesStatus::Partial);
        assert!(bundle
            .series_unavailable_reason
            .as_deref()
            .is_some_and(|value| value.contains("log-derived")));
        assert!(bundle
            .source_artifacts
            .iter()
            .any(|artifact| artifact.source_kind
                == RemoteTrainingArtifactSourceKind::LogDerivedFallback));
    }

    #[test]
    fn runtime_env_detection_uses_runpod_defaults_when_runpod_env_is_present() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let report_path = temp_dir
            .path()
            .join("parameter_golf_single_h100_training.json");
        unsafe { std::env::set_var(RUNPOD_POD_ID_ENV, "pod-123") };
        let metadata = ParameterGolfSingleH100VisualizationMetadata::from_runtime_env(
            "run-1",
            report_path.as_path(),
        )
        .expect("metadata parse")
        .expect("runpod metadata");
        unsafe { std::env::remove_var(RUNPOD_POD_ID_ENV) };

        assert_eq!(metadata.provider, RemoteTrainingProvider::RunPod);
        assert_eq!(metadata.profile_id, "runpod_h100_single_gpu");
        assert_eq!(metadata.lane_id, "parameter_golf.runpod_single_h100");
    }
}
