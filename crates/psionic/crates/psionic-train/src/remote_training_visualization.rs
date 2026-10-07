use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Stable schema version for the provider-neutral remote-training bundle.
pub const REMOTE_TRAINING_VISUALIZATION_BUNDLE_SCHEMA_VERSION: &str =
    "psionic.remote_training_visualization_bundle.v1";
/// Stable schema version for the provider-neutral remote-training run index.
pub const REMOTE_TRAINING_RUN_INDEX_SCHEMA_VERSION: &str = "psionic.remote_training_run_index.v1";
/// Default one-second UI refresh target for active runs.
pub const REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS: u64 = 1_000;

/// Provider family that produced one remote training run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingProvider {
    GoogleCloud,
    RunPod,
    LocalHybrid,
}

/// Artifact-emission mode exposed to the app.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingEmissionMode {
    Stream,
    AppendOnlySnapshots,
    PostRunOnly,
}

/// Truth posture for the primary chartable training series.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingSeriesStatus {
    Available,
    Partial,
    Unavailable,
}

/// Current result classification for one run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingResultClassification {
    Planned,
    Active,
    CompletedSuccess,
    CompletedFailure,
    Refused,
    RehearsalOnly,
}

/// Severity level for one surfaced event row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingEventSeverity {
    Info,
    Warning,
    Error,
}

/// Provenance posture for one source artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingArtifactSourceKind {
    RuntimeOwned,
    FinalizerOwned,
    ProviderGenerated,
    LogDerivedFallback,
    LocalMirror,
}

/// Refresh contract surfaced to the app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingRefreshContract {
    /// Target UI refresh interval in milliseconds.
    pub target_ui_update_interval_ms: u64,
    /// Emission posture for the retained bundle.
    pub emission_mode: RemoteTrainingEmissionMode,
    /// Last observed heartbeat timestamp when the artifact was sealed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at_ms: Option<u64>,
    /// Monotonic heartbeat sequence observed by the artifact writer.
    pub heartbeat_seq: u64,
}

/// Lifecycle transition or major phase boundary.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingTimelineEntry {
    /// Observed timestamp in milliseconds since epoch.
    pub observed_at_ms: u64,
    /// Current top-level phase.
    pub phase: String,
    /// Optional narrower subphase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subphase: Option<String>,
    /// Short explanation of the transition.
    pub detail: String,
}

/// One one-second heartbeat row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingHeartbeatSample {
    /// Observed timestamp in milliseconds since epoch.
    pub observed_at_ms: u64,
    /// Current top-level phase.
    pub phase: String,
    /// Optional narrower subphase.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subphase: Option<String>,
    /// Current optimizer step when one is in flight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step_in_progress: Option<u64>,
    /// Current microbatch ordinal when one is in flight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub microbatch_in_progress: Option<u32>,
    /// Active subsystems that explain what the runtime is doing now.
    pub active_subsystems: Vec<String>,
    /// The app should mark the run stale after this many milliseconds without a
    /// new heartbeat.
    pub stale_after_ms: u64,
}

/// Primary loss-curve sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingLossSample {
    /// Global optimizer step when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_step: Option<u64>,
    /// Elapsed runtime milliseconds from run start.
    pub elapsed_ms: u64,
    /// Train loss when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub train_loss: Option<f32>,
    /// EMA loss when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ema_loss: Option<f32>,
    /// Validation loss when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation_loss: Option<f32>,
}

/// Optimizer and model-math sample.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingMathSample {
    /// Observed timestamp in milliseconds since epoch.
    pub observed_at_ms: u64,
    /// Global optimizer step when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global_step: Option<u64>,
    /// Learning rate when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learning_rate: Option<f32>,
    /// Gradient norm when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient_norm: Option<f32>,
    /// Parameter norm when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_norm: Option<f32>,
    /// Update norm when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_norm: Option<f32>,
    /// Fraction of clipped elements or clipped groups when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_fraction: Option<f32>,
    /// Count of clipping events when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip_event_count: Option<u32>,
    /// Mixed-precision loss scale when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loss_scale: Option<f32>,
    /// Number of non-finite values detected by the runtime.
    pub non_finite_count: u32,
    /// Bounded model-specific diagnostics that stay honest and lightweight.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model_specific_diagnostics: BTreeMap<String, f32>,
}

/// Runtime pipeline and throughput sample.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingRuntimeSample {
    /// Observed timestamp in milliseconds since epoch.
    pub observed_at_ms: u64,
    /// Time spent waiting on the dataloader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_wait_ms: Option<u64>,
    /// Forward-pass time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward_ms: Option<u64>,
    /// Backward-pass time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backward_ms: Option<u64>,
    /// Optimizer time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optimizer_ms: Option<u64>,
    /// Checkpoint-write time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_ms: Option<u64>,
    /// Evaluation-pass time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_ms: Option<u64>,
    /// Current token throughput when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_per_second: Option<u64>,
    /// Current sample throughput in milli-samples per second when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub samples_per_second_milli: Option<u32>,
}

/// Per-device telemetry sample.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingGpuSample {
    /// Observed timestamp in milliseconds since epoch.
    pub observed_at_ms: u64,
    /// Stable device identifier.
    pub device_id: String,
    /// Human-readable device label.
    pub device_label: String,
    /// GPU utilization in basis points.
    pub utilization_bps: u32,
    /// Used memory in bytes.
    pub memory_used_bytes: u64,
    /// Total memory in bytes.
    pub memory_total_bytes: u64,
    /// Temperature when exported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature_celsius: Option<u16>,
    /// Power draw when exported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_watts: Option<u16>,
}

/// Distributed or collective telemetry sample.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingDistributedSample {
    /// Observed timestamp in milliseconds since epoch.
    pub observed_at_ms: u64,
    /// Number of active ranks observed by the coordinator.
    pub participating_rank_count: u16,
    /// Rank skew in milliseconds when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank_skew_ms: Option<u64>,
    /// Slowest-rank duration in milliseconds when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slowest_rank_ms: Option<u64>,
    /// Collective time in milliseconds when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collective_ms: Option<u64>,
    /// Number of stalled ranks currently detected.
    pub stalled_rank_count: u16,
}

/// Typed recent-event row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingEventSample {
    /// Observed timestamp in milliseconds since epoch.
    pub observed_at_ms: u64,
    /// Event severity.
    pub severity: RemoteTrainingEventSeverity,
    /// Stable event kind or type.
    pub event_kind: String,
    /// Human-readable detail.
    pub detail: String,
}

/// Provenance for one retained source artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingSourceArtifact {
    /// Artifact role inside the bundle.
    pub artifact_role: String,
    /// Provider or local mirror URI.
    pub artifact_uri: String,
    /// Artifact digest when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    /// Source kind that produced the artifact.
    pub source_kind: RemoteTrainingArtifactSourceKind,
    /// Whether the artifact is authoritative for the surfaced metric family.
    pub authoritative: bool,
    /// Source receipt identifiers when present.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_receipt_ids: Vec<String>,
    /// Short explanation of why this artifact is surfaced.
    pub detail: String,
}

/// Summary card payload for one run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingVisualizationSummary {
    /// Total completed optimizer steps.
    pub total_steps_completed: u64,
    /// Latest observed optimizer step when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_global_step: Option<u64>,
    /// Latest observed train loss when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_train_loss: Option<f32>,
    /// Latest observed EMA loss when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_ema_loss: Option<f32>,
    /// Latest observed validation loss when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_validation_loss: Option<f32>,
    /// Latest observed token throughput when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_tokens_per_second: Option<u64>,
    /// Latest observed sample throughput in milli-samples per second when
    /// known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_samples_per_second_milli: Option<u32>,
    /// Accumulated cost when surfaced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accumulated_cost_microusd: Option<u64>,
    /// Latest checkpoint ref when surfaced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_checkpoint_ref: Option<String>,
    /// Short explanation of the current summary posture.
    pub detail: String,
}

/// Provider-neutral app-facing remote-training bundle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingVisualizationBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Provider family.
    pub provider: RemoteTrainingProvider,
    /// Provider or launch profile identifier.
    pub profile_id: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Repo revision or artifact revision string.
    pub repo_revision: String,
    /// Current result classification for the run.
    pub result_classification: RemoteTrainingResultClassification,
    /// Refresh posture surfaced to the app.
    pub refresh_contract: RemoteTrainingRefreshContract,
    /// Availability posture for the primary chartable training series.
    pub series_status: RemoteTrainingSeriesStatus,
    /// Explicit reason when the primary chartable training series is partial or
    /// unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_unavailable_reason: Option<String>,
    /// Coarse lifecycle timeline.
    pub timeline: Vec<RemoteTrainingTimelineEntry>,
    /// Summary card payload.
    pub summary: RemoteTrainingVisualizationSummary,
    /// One-second heartbeat series.
    pub heartbeat_series: Vec<RemoteTrainingHeartbeatSample>,
    /// Primary loss series.
    pub loss_series: Vec<RemoteTrainingLossSample>,
    /// Optimizer and model-math telemetry.
    pub math_series: Vec<RemoteTrainingMathSample>,
    /// Runtime pipeline telemetry.
    pub runtime_series: Vec<RemoteTrainingRuntimeSample>,
    /// Device telemetry.
    pub gpu_series: Vec<RemoteTrainingGpuSample>,
    /// Distributed telemetry.
    pub distributed_series: Vec<RemoteTrainingDistributedSample>,
    /// Typed event stream.
    pub event_series: Vec<RemoteTrainingEventSample>,
    /// Source provenance.
    pub source_artifacts: Vec<RemoteTrainingSourceArtifact>,
    /// Stable digest over the bundle payload.
    pub bundle_digest: String,
}

impl RemoteTrainingVisualizationBundle {
    /// Validates the bundle and fills the stable digest.
    pub fn finalize(mut self) -> Result<Self, RemoteTrainingVisualizationError> {
        self.schema_version = String::from(REMOTE_TRAINING_VISUALIZATION_BUNDLE_SCHEMA_VERSION);
        self.bundle_digest.clear();
        self.validate_payload()?;
        self.bundle_digest = stable_bundle_digest(&self);
        Ok(self)
    }

    /// Validates one retained bundle, including the stable digest.
    pub fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        self.validate_payload()?;
        if self.bundle_digest != stable_bundle_digest(self) {
            return Err(RemoteTrainingVisualizationError::DigestMismatch {
                kind: String::from("remote_training_visualization_bundle"),
            });
        }
        Ok(())
    }

    fn validate_payload(&self) -> Result<(), RemoteTrainingVisualizationError> {
        check_string_match(
            self.schema_version.as_str(),
            REMOTE_TRAINING_VISUALIZATION_BUNDLE_SCHEMA_VERSION,
            "remote_training_visualization_bundle.schema_version",
        )?;
        ensure_nonempty(
            self.bundle_id.as_str(),
            "remote_training_visualization_bundle.bundle_id",
        )?;
        ensure_nonempty(
            self.profile_id.as_str(),
            "remote_training_visualization_bundle.profile_id",
        )?;
        ensure_nonempty(
            self.lane_id.as_str(),
            "remote_training_visualization_bundle.lane_id",
        )?;
        ensure_nonempty(
            self.run_id.as_str(),
            "remote_training_visualization_bundle.run_id",
        )?;
        ensure_nonempty(
            self.repo_revision.as_str(),
            "remote_training_visualization_bundle.repo_revision",
        )?;
        self.refresh_contract.validate(self.result_classification)?;
        self.summary.validate()?;
        validate_optional_nonempty(
            self.series_unavailable_reason.as_deref(),
            "remote_training_visualization_bundle.series_unavailable_reason",
        )?;
        match self.series_status {
            RemoteTrainingSeriesStatus::Available => {
                if self.series_unavailable_reason.is_some() {
                    return Err(RemoteTrainingVisualizationError::InvalidValue {
                        field: String::from(
                            "remote_training_visualization_bundle.series_unavailable_reason",
                        ),
                        detail: String::from(
                            "available series must not carry an unavailable reason",
                        ),
                    });
                }
                if self.loss_series.is_empty() {
                    return Err(RemoteTrainingVisualizationError::InvalidValue {
                        field: String::from("remote_training_visualization_bundle.loss_series"),
                        detail: String::from(
                            "available series_status requires at least one loss-series sample",
                        ),
                    });
                }
            }
            RemoteTrainingSeriesStatus::Partial | RemoteTrainingSeriesStatus::Unavailable => {
                if self.series_unavailable_reason.is_none() {
                    return Err(RemoteTrainingVisualizationError::MissingField {
                        field: String::from(
                            "remote_training_visualization_bundle.series_unavailable_reason",
                        ),
                    });
                }
            }
        }
        validate_monotonic_timeline(self.timeline.as_slice())?;
        validate_monotonic_timestamps(
            self.heartbeat_series
                .iter()
                .map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle.heartbeat_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.loss_series.iter().map(|sample| sample.elapsed_ms),
            "remote_training_visualization_bundle.loss_series.elapsed_ms",
        )?;
        validate_monotonic_timestamps(
            self.math_series.iter().map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle.math_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.runtime_series
                .iter()
                .map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle.runtime_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.gpu_series.iter().map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle.gpu_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.distributed_series
                .iter()
                .map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle.distributed_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.event_series.iter().map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle.event_series.observed_at_ms",
        )?;
        for entry in &self.timeline {
            entry.validate()?;
        }
        for sample in &self.heartbeat_series {
            sample.validate()?;
        }
        for sample in &self.loss_series {
            sample.validate()?;
        }
        for sample in &self.math_series {
            sample.validate()?;
        }
        for sample in &self.runtime_series {
            sample.validate()?;
        }
        for sample in &self.gpu_series {
            sample.validate()?;
        }
        for sample in &self.distributed_series {
            sample.validate()?;
        }
        for sample in &self.event_series {
            sample.validate()?;
        }
        let mut seen_roles = BTreeSet::new();
        for artifact in &self.source_artifacts {
            artifact.validate()?;
            if !seen_roles.insert(artifact.artifact_role.as_str()) {
                return Err(RemoteTrainingVisualizationError::DuplicateValue {
                    field: String::from(
                        "remote_training_visualization_bundle.source_artifacts.artifact_role",
                    ),
                    value: artifact.artifact_role.clone(),
                });
            }
        }
        Ok(())
    }
}

impl RemoteTrainingRefreshContract {
    pub(crate) fn validate(
        &self,
        result_classification: RemoteTrainingResultClassification,
    ) -> Result<(), RemoteTrainingVisualizationError> {
        if self.target_ui_update_interval_ms == 0 {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(
                    "remote_training_visualization_bundle.refresh_contract.target_ui_update_interval_ms",
                ),
                detail: String::from("refresh interval must be positive"),
            });
        }
        if result_classification == RemoteTrainingResultClassification::Active
            && self.emission_mode == RemoteTrainingEmissionMode::PostRunOnly
        {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(
                    "remote_training_visualization_bundle.refresh_contract.emission_mode",
                ),
                detail: String::from(
                    "active runs cannot be post-run-only if the app must stay alive every second",
                ),
            });
        }
        if result_classification == RemoteTrainingResultClassification::Active
            && self.target_ui_update_interval_ms > REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS
        {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(
                    "remote_training_visualization_bundle.refresh_contract.target_ui_update_interval_ms",
                ),
                detail: format!(
                    "active runs must target {} ms or faster app refresh",
                    REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS
                ),
            });
        }
        if self.heartbeat_seq == 0 && self.last_heartbeat_at_ms.is_some() {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(
                    "remote_training_visualization_bundle.refresh_contract.heartbeat_seq",
                ),
                detail: String::from("heartbeat sequence must be positive when a heartbeat exists"),
            });
        }
        Ok(())
    }
}

impl RemoteTrainingTimelineEntry {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.phase.as_str(),
            "remote_training_visualization_bundle.timeline.phase",
        )?;
        validate_optional_nonempty(
            self.subphase.as_deref(),
            "remote_training_visualization_bundle.timeline.subphase",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "remote_training_visualization_bundle.timeline.detail",
        )?;
        Ok(())
    }
}

impl RemoteTrainingHeartbeatSample {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.phase.as_str(),
            "remote_training_visualization_bundle.heartbeat_series.phase",
        )?;
        validate_optional_nonempty(
            self.subphase.as_deref(),
            "remote_training_visualization_bundle.heartbeat_series.subphase",
        )?;
        if self.active_subsystems.is_empty() {
            return Err(RemoteTrainingVisualizationError::MissingField {
                field: String::from(
                    "remote_training_visualization_bundle.heartbeat_series.active_subsystems",
                ),
            });
        }
        reject_duplicate_strings(
            self.active_subsystems.as_slice(),
            "remote_training_visualization_bundle.heartbeat_series.active_subsystems",
        )?;
        if self.stale_after_ms == 0 {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(
                    "remote_training_visualization_bundle.heartbeat_series.stale_after_ms",
                ),
                detail: String::from("stale_after_ms must be positive"),
            });
        }
        Ok(())
    }
}

impl RemoteTrainingLossSample {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        if self.train_loss.is_none() && self.ema_loss.is_none() && self.validation_loss.is_none() {
            return Err(RemoteTrainingVisualizationError::MissingField {
                field: String::from(
                    "remote_training_visualization_bundle.loss_series.train_loss_or_ema_loss_or_validation_loss",
                ),
            });
        }
        validate_optional_metric(
            self.train_loss,
            "remote_training_visualization_bundle.loss_series.train_loss",
        )?;
        validate_optional_metric(
            self.ema_loss,
            "remote_training_visualization_bundle.loss_series.ema_loss",
        )?;
        validate_optional_metric(
            self.validation_loss,
            "remote_training_visualization_bundle.loss_series.validation_loss",
        )?;
        Ok(())
    }
}

impl RemoteTrainingMathSample {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        validate_optional_metric(
            self.learning_rate,
            "remote_training_visualization_bundle.math_series.learning_rate",
        )?;
        validate_optional_metric(
            self.gradient_norm,
            "remote_training_visualization_bundle.math_series.gradient_norm",
        )?;
        validate_optional_metric(
            self.parameter_norm,
            "remote_training_visualization_bundle.math_series.parameter_norm",
        )?;
        validate_optional_metric(
            self.update_norm,
            "remote_training_visualization_bundle.math_series.update_norm",
        )?;
        validate_optional_fraction(
            self.clip_fraction,
            "remote_training_visualization_bundle.math_series.clip_fraction",
        )?;
        validate_optional_metric(
            self.loss_scale,
            "remote_training_visualization_bundle.math_series.loss_scale",
        )?;
        for (key, value) in &self.model_specific_diagnostics {
            ensure_nonempty(
                key.as_str(),
                "remote_training_visualization_bundle.math_series.model_specific_diagnostics.key",
            )?;
            validate_metric(
                *value,
                "remote_training_visualization_bundle.math_series.model_specific_diagnostics.value",
            )?;
        }
        Ok(())
    }
}

impl RemoteTrainingRuntimeSample {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        validate_optional_duration(
            self.data_wait_ms,
            "remote_training_visualization_bundle.runtime_series.data_wait_ms",
        )?;
        validate_optional_duration(
            self.forward_ms,
            "remote_training_visualization_bundle.runtime_series.forward_ms",
        )?;
        validate_optional_duration(
            self.backward_ms,
            "remote_training_visualization_bundle.runtime_series.backward_ms",
        )?;
        validate_optional_duration(
            self.optimizer_ms,
            "remote_training_visualization_bundle.runtime_series.optimizer_ms",
        )?;
        validate_optional_duration(
            self.checkpoint_ms,
            "remote_training_visualization_bundle.runtime_series.checkpoint_ms",
        )?;
        validate_optional_duration(
            self.evaluation_ms,
            "remote_training_visualization_bundle.runtime_series.evaluation_ms",
        )?;
        Ok(())
    }
}

impl RemoteTrainingGpuSample {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.device_id.as_str(),
            "remote_training_visualization_bundle.gpu_series.device_id",
        )?;
        ensure_nonempty(
            self.device_label.as_str(),
            "remote_training_visualization_bundle.gpu_series.device_label",
        )?;
        if self.utilization_bps > 10_000 {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(
                    "remote_training_visualization_bundle.gpu_series.utilization_bps",
                ),
                detail: String::from("utilization_bps must stay inside 0..=10000"),
            });
        }
        if self.memory_total_bytes == 0 || self.memory_used_bytes > self.memory_total_bytes {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from("remote_training_visualization_bundle.gpu_series.memory_bytes"),
                detail: String::from(
                    "memory_total_bytes must be positive and memory_used_bytes must stay within it",
                ),
            });
        }
        Ok(())
    }
}

impl RemoteTrainingDistributedSample {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        if self.participating_rank_count == 0 {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(
                    "remote_training_visualization_bundle.distributed_series.participating_rank_count",
                ),
                detail: String::from("participating_rank_count must be positive"),
            });
        }
        if self.stalled_rank_count > self.participating_rank_count {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(
                    "remote_training_visualization_bundle.distributed_series.stalled_rank_count",
                ),
                detail: String::from("stalled_rank_count must not exceed participating_rank_count"),
            });
        }
        Ok(())
    }
}

impl RemoteTrainingEventSample {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.event_kind.as_str(),
            "remote_training_visualization_bundle.event_series.event_kind",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "remote_training_visualization_bundle.event_series.detail",
        )?;
        Ok(())
    }
}

impl RemoteTrainingSourceArtifact {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.artifact_role.as_str(),
            "remote_training_visualization_bundle.source_artifacts.artifact_role",
        )?;
        ensure_nonempty(
            self.artifact_uri.as_str(),
            "remote_training_visualization_bundle.source_artifacts.artifact_uri",
        )?;
        validate_optional_nonempty(
            self.artifact_digest.as_deref(),
            "remote_training_visualization_bundle.source_artifacts.artifact_digest",
        )?;
        reject_duplicate_strings(
            self.source_receipt_ids.as_slice(),
            "remote_training_visualization_bundle.source_artifacts.source_receipt_ids",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "remote_training_visualization_bundle.source_artifacts.detail",
        )?;
        Ok(())
    }
}

impl RemoteTrainingVisualizationSummary {
    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        if self.total_steps_completed > 0 && self.latest_global_step.is_none() {
            return Err(RemoteTrainingVisualizationError::MissingField {
                field: String::from(
                    "remote_training_visualization_bundle.summary.latest_global_step",
                ),
            });
        }
        validate_optional_metric(
            self.latest_train_loss,
            "remote_training_visualization_bundle.summary.latest_train_loss",
        )?;
        validate_optional_metric(
            self.latest_ema_loss,
            "remote_training_visualization_bundle.summary.latest_ema_loss",
        )?;
        validate_optional_metric(
            self.latest_validation_loss,
            "remote_training_visualization_bundle.summary.latest_validation_loss",
        )?;
        validate_optional_nonempty(
            self.latest_checkpoint_ref.as_deref(),
            "remote_training_visualization_bundle.summary.latest_checkpoint_ref",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "remote_training_visualization_bundle.summary.detail",
        )?;
        Ok(())
    }
}

/// One run-list entry for the app-facing index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingRunIndexEntry {
    /// Provider family.
    pub provider: RemoteTrainingProvider,
    /// Provider or launch profile identifier.
    pub profile_id: String,
    /// Stable lane identifier.
    pub lane_id: String,
    /// Stable run identifier.
    pub run_id: String,
    /// Repo revision or artifact revision string.
    pub repo_revision: String,
    /// Current result classification for the run.
    pub result_classification: RemoteTrainingResultClassification,
    /// Availability posture for the primary chartable training series.
    pub series_status: RemoteTrainingSeriesStatus,
    /// Explicit reason when the primary series is partial or unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_unavailable_reason: Option<String>,
    /// Last observed heartbeat timestamp for active runs when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at_ms: Option<u64>,
    /// Local or provider bundle artifact URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_artifact_uri: Option<String>,
    /// Bundle digest when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
    /// Short display label used in the run list.
    pub summary_label: String,
    /// Short explanation of the entry.
    pub detail: String,
}

impl RemoteTrainingRunIndexEntry {
    fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.profile_id.as_str(),
            "remote_training_run_index.entries.profile_id",
        )?;
        ensure_nonempty(
            self.lane_id.as_str(),
            "remote_training_run_index.entries.lane_id",
        )?;
        ensure_nonempty(
            self.run_id.as_str(),
            "remote_training_run_index.entries.run_id",
        )?;
        ensure_nonempty(
            self.repo_revision.as_str(),
            "remote_training_run_index.entries.repo_revision",
        )?;
        ensure_nonempty(
            self.summary_label.as_str(),
            "remote_training_run_index.entries.summary_label",
        )?;
        ensure_nonempty(
            self.detail.as_str(),
            "remote_training_run_index.entries.detail",
        )?;
        validate_optional_nonempty(
            self.series_unavailable_reason.as_deref(),
            "remote_training_run_index.entries.series_unavailable_reason",
        )?;
        validate_optional_nonempty(
            self.bundle_artifact_uri.as_deref(),
            "remote_training_run_index.entries.bundle_artifact_uri",
        )?;
        validate_optional_nonempty(
            self.bundle_digest.as_deref(),
            "remote_training_run_index.entries.bundle_digest",
        )?;
        match self.series_status {
            RemoteTrainingSeriesStatus::Available => {
                if self.series_unavailable_reason.is_some() {
                    return Err(RemoteTrainingVisualizationError::InvalidValue {
                        field: String::from(
                            "remote_training_run_index.entries.series_unavailable_reason",
                        ),
                        detail: String::from(
                            "available series must not carry an unavailable reason",
                        ),
                    });
                }
            }
            RemoteTrainingSeriesStatus::Partial | RemoteTrainingSeriesStatus::Unavailable => {
                if self.series_unavailable_reason.is_none() {
                    return Err(RemoteTrainingVisualizationError::MissingField {
                        field: String::from(
                            "remote_training_run_index.entries.series_unavailable_reason",
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Run-list surface that enumerates remote training runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingRunIndex {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable index identifier.
    pub index_id: String,
    /// Index generation timestamp in milliseconds since epoch.
    pub generated_at_ms: u64,
    /// Ordered run entries.
    pub entries: Vec<RemoteTrainingRunIndexEntry>,
    /// Short explanation of the index posture.
    pub detail: String,
    /// Stable digest over the index payload.
    pub index_digest: String,
}

impl RemoteTrainingRunIndex {
    /// Validates the run index and fills the stable digest.
    pub fn finalize(mut self) -> Result<Self, RemoteTrainingVisualizationError> {
        self.schema_version = String::from(REMOTE_TRAINING_RUN_INDEX_SCHEMA_VERSION);
        self.index_digest.clear();
        self.validate_payload()?;
        self.index_digest = stable_run_index_digest(&self);
        Ok(self)
    }

    /// Validates one retained run index, including the stable digest.
    pub fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        self.validate_payload()?;
        if self.index_digest != stable_run_index_digest(self) {
            return Err(RemoteTrainingVisualizationError::DigestMismatch {
                kind: String::from("remote_training_run_index"),
            });
        }
        Ok(())
    }

    fn validate_payload(&self) -> Result<(), RemoteTrainingVisualizationError> {
        check_string_match(
            self.schema_version.as_str(),
            REMOTE_TRAINING_RUN_INDEX_SCHEMA_VERSION,
            "remote_training_run_index.schema_version",
        )?;
        ensure_nonempty(self.index_id.as_str(), "remote_training_run_index.index_id")?;
        ensure_nonempty(self.detail.as_str(), "remote_training_run_index.detail")?;
        if self.entries.is_empty() {
            return Err(RemoteTrainingVisualizationError::MissingField {
                field: String::from("remote_training_run_index.entries"),
            });
        }
        let mut seen_runs = BTreeSet::new();
        for entry in &self.entries {
            entry.validate()?;
            let key = format!(
                "{:?}|{}|{}|{}",
                entry.provider, entry.profile_id, entry.lane_id, entry.run_id
            );
            if !seen_runs.insert(key.clone()) {
                return Err(RemoteTrainingVisualizationError::DuplicateValue {
                    field: String::from("remote_training_run_index.entries.run_identity"),
                    value: key,
                });
            }
        }
        Ok(())
    }
}

/// Builds one provider-neutral bundle and computes its digest.
pub fn record_remote_training_visualization_bundle(
    bundle: RemoteTrainingVisualizationBundle,
) -> Result<RemoteTrainingVisualizationBundle, RemoteTrainingVisualizationError> {
    bundle.finalize()
}

/// Builds one provider-neutral run index and computes its digest.
pub fn build_remote_training_run_index(
    index: RemoteTrainingRunIndex,
) -> Result<RemoteTrainingRunIndex, RemoteTrainingVisualizationError> {
    index.finalize()
}

/// Returns a canonical full-series sample bundle for the PGOLF single-H100
/// lane.
pub fn sample_parameter_golf_live_visualization_bundle(
) -> Result<RemoteTrainingVisualizationBundle, RemoteTrainingVisualizationError> {
    record_remote_training_visualization_bundle(RemoteTrainingVisualizationBundle {
        schema_version: String::new(),
        bundle_id: String::from("parameter-golf-runpod-single-h100-live-sample-v1"),
        provider: RemoteTrainingProvider::RunPod,
        profile_id: String::from("runpod_h100_single_gpu"),
        lane_id: String::from("parameter_golf.runpod_single_h100"),
        run_id: String::from("parameter-golf-runpod-single-h100-live-sample"),
        repo_revision: String::from("main@5e6f5f0f"),
        result_classification: RemoteTrainingResultClassification::Active,
        refresh_contract: RemoteTrainingRefreshContract {
            target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
            emission_mode: RemoteTrainingEmissionMode::AppendOnlySnapshots,
            last_heartbeat_at_ms: Some(1_742_846_402_000),
            heartbeat_seq: 5,
        },
        series_status: RemoteTrainingSeriesStatus::Available,
        series_unavailable_reason: None,
        timeline: vec![
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_395_000,
                phase: String::from("provisioning"),
                subphase: Some(String::from("pod_ready")),
                detail: String::from(
                    "RunPod pod admission completed and the trainer payload was staged locally.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_397_000,
                phase: String::from("training"),
                subphase: Some(String::from("warmup")),
                detail: String::from(
                    "The bounded single-H100 trainer entered warmup and started append-only live snapshots.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_400_000,
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                detail: String::from(
                    "The first measured optimizer steps completed under the live snapshot contract.",
                ),
            },
        ],
        summary: RemoteTrainingVisualizationSummary {
            total_steps_completed: 3,
            latest_global_step: Some(3),
            latest_train_loss: Some(3.982),
            latest_ema_loss: Some(4.031),
            latest_validation_loss: Some(4.227),
            latest_tokens_per_second: Some(143_360),
            latest_samples_per_second_milli: Some(140),
            accumulated_cost_microusd: Some(3_200_000),
            latest_checkpoint_ref: Some(String::from(
                "checkpoint://parameter-golf/runpod-single-h100/step-3",
            )),
            detail: String::from(
                "This sample shows the full-series shape expected from the always-live single-H100 lane.",
            ),
        },
        heartbeat_series: vec![
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_400_000,
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                step_in_progress: Some(1),
                microbatch_in_progress: Some(8),
                active_subsystems: vec![
                    String::from("dataloader"),
                    String::from("forward"),
                    String::from("optimizer"),
                ],
                stale_after_ms: 2_500,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_401_000,
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                step_in_progress: Some(2),
                microbatch_in_progress: Some(8),
                active_subsystems: vec![
                    String::from("forward"),
                    String::from("backward"),
                    String::from("optimizer"),
                ],
                stale_after_ms: 2_500,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_402_000,
                phase: String::from("training"),
                subphase: Some(String::from("validation")),
                step_in_progress: Some(3),
                microbatch_in_progress: Some(0),
                active_subsystems: vec![
                    String::from("validation"),
                    String::from("checkpoint"),
                    String::from("gpu_sampling"),
                ],
                stale_after_ms: 2_500,
            },
        ],
        loss_series: vec![
            RemoteTrainingLossSample {
                global_step: Some(1),
                elapsed_ms: 1_000,
                train_loss: Some(4.181),
                ema_loss: Some(4.181),
                validation_loss: None,
            },
            RemoteTrainingLossSample {
                global_step: Some(2),
                elapsed_ms: 2_000,
                train_loss: Some(4.024),
                ema_loss: Some(4.102),
                validation_loss: None,
            },
            RemoteTrainingLossSample {
                global_step: Some(3),
                elapsed_ms: 3_000,
                train_loss: Some(3.982),
                ema_loss: Some(4.031),
                validation_loss: Some(4.227),
            },
        ],
        math_series: vec![
            RemoteTrainingMathSample {
                observed_at_ms: 1_742_846_400_000,
                global_step: Some(1),
                learning_rate: Some(0.0006),
                gradient_norm: Some(1.41),
                parameter_norm: Some(73.4),
                update_norm: Some(0.84),
                clip_fraction: Some(0.0),
                clip_event_count: Some(0),
                loss_scale: Some(1024.0),
                non_finite_count: 0,
                model_specific_diagnostics: BTreeMap::from([
                    (String::from("logit_entropy"), 5.42),
                    (String::from("attention_entropy"), 2.81),
                ]),
            },
            RemoteTrainingMathSample {
                observed_at_ms: 1_742_846_401_000,
                global_step: Some(2),
                learning_rate: Some(0.0006),
                gradient_norm: Some(1.35),
                parameter_norm: Some(73.5),
                update_norm: Some(0.81),
                clip_fraction: Some(0.0),
                clip_event_count: Some(0),
                loss_scale: Some(1024.0),
                non_finite_count: 0,
                model_specific_diagnostics: BTreeMap::from([
                    (String::from("logit_entropy"), 5.38),
                    (String::from("attention_entropy"), 2.77),
                ]),
            },
            RemoteTrainingMathSample {
                observed_at_ms: 1_742_846_402_000,
                global_step: Some(3),
                learning_rate: Some(0.0006),
                gradient_norm: Some(1.29),
                parameter_norm: Some(73.6),
                update_norm: Some(0.79),
                clip_fraction: Some(0.0),
                clip_event_count: Some(0),
                loss_scale: Some(1024.0),
                non_finite_count: 0,
                model_specific_diagnostics: BTreeMap::from([
                    (String::from("logit_entropy"), 5.31),
                    (String::from("attention_entropy"), 2.72),
                ]),
            },
        ],
        runtime_series: vec![
            RemoteTrainingRuntimeSample {
                observed_at_ms: 1_742_846_400_000,
                data_wait_ms: Some(41),
                forward_ms: Some(318),
                backward_ms: Some(374),
                optimizer_ms: Some(118),
                checkpoint_ms: None,
                evaluation_ms: None,
                tokens_per_second: Some(141_824),
                samples_per_second_milli: Some(138),
            },
            RemoteTrainingRuntimeSample {
                observed_at_ms: 1_742_846_401_000,
                data_wait_ms: Some(38),
                forward_ms: Some(311),
                backward_ms: Some(365),
                optimizer_ms: Some(116),
                checkpoint_ms: None,
                evaluation_ms: None,
                tokens_per_second: Some(142_976),
                samples_per_second_milli: Some(139),
            },
            RemoteTrainingRuntimeSample {
                observed_at_ms: 1_742_846_402_000,
                data_wait_ms: Some(34),
                forward_ms: Some(307),
                backward_ms: Some(360),
                optimizer_ms: Some(114),
                checkpoint_ms: Some(88),
                evaluation_ms: Some(205),
                tokens_per_second: Some(143_360),
                samples_per_second_milli: Some(140),
            },
        ],
        gpu_series: vec![
            RemoteTrainingGpuSample {
                observed_at_ms: 1_742_846_400_000,
                device_id: String::from("cuda:0"),
                device_label: String::from("NVIDIA H100 80GB"),
                utilization_bps: 9_620,
                memory_used_bytes: 64 * 1024 * 1024 * 1024,
                memory_total_bytes: 80 * 1024 * 1024 * 1024,
                temperature_celsius: Some(67),
                power_watts: Some(335),
            },
            RemoteTrainingGpuSample {
                observed_at_ms: 1_742_846_401_000,
                device_id: String::from("cuda:0"),
                device_label: String::from("NVIDIA H100 80GB"),
                utilization_bps: 9_710,
                memory_used_bytes: 64 * 1024 * 1024 * 1024 + 268_435_456,
                memory_total_bytes: 80 * 1024 * 1024 * 1024,
                temperature_celsius: Some(68),
                power_watts: Some(339),
            },
            RemoteTrainingGpuSample {
                observed_at_ms: 1_742_846_402_000,
                device_id: String::from("cuda:0"),
                device_label: String::from("NVIDIA H100 80GB"),
                utilization_bps: 9_580,
                memory_used_bytes: 64 * 1024 * 1024 * 1024 + 536_870_912,
                memory_total_bytes: 80 * 1024 * 1024 * 1024,
                temperature_celsius: Some(68),
                power_watts: Some(341),
            },
        ],
        distributed_series: Vec::new(),
        event_series: vec![
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_846_397_000,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("trainer_ready"),
                detail: String::from(
                    "The single-H100 trainer materialized its append-only live snapshot contract.",
                ),
            },
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_846_401_000,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("optimizer_step_completed"),
                detail: String::from("Optimizer step 2 completed without overflow or clipping."),
            },
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_846_402_000,
                severity: RemoteTrainingEventSeverity::Warning,
                event_kind: String::from("validation_started"),
                detail: String::from(
                    "Validation and checkpoint flush started while the run remained active.",
                ),
            },
        ],
        source_artifacts: vec![
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("live_bundle"),
                artifact_uri: String::from(
                    "local-mirror://parameter-golf/runpod-single-h100/remote_training_visualization_bundle.json",
                ),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf.runpod_single_h100.live_snapshot_stream.v1",
                )],
                detail: String::from(
                    "The app should read the normalized local mirror instead of provider-shaped logs.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("trainer_report"),
                artifact_uri: String::from(
                    "runpod://parameter-golf/runpod-single-h100/parameter_golf_single_h100_training.json",
                ),
                artifact_digest: Some(String::from(
                    "d7c39fb41957551adf0c1ec1bf7aa8241ba0b766e914cf8f4d58f7f5735c28a4",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf_single_h100_training_report.v1",
                )],
                detail: String::from(
                    "Trainer-owned JSON remains authoritative for loss, math, and runtime telemetry.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("gpu_samples"),
                artifact_uri: String::from(
                    "runpod://parameter-golf/runpod-single-h100/nvidia-smi-gpu-samples.csv",
                ),
                artifact_digest: Some(String::from(
                    "f72e7c98f98d1aa627399831d4ab90c3fc42f8aaad719996257053b956f95bbb",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::ProviderGenerated,
                authoritative: true,
                source_receipt_ids: Vec::new(),
                detail: String::from(
                    "Provider GPU samples remain the authoritative device source for the lane.",
                ),
            },
        ],
        bundle_digest: String::new(),
    })
}

/// Returns a canonical summary-only sample bundle for the existing Google
/// single-node Psion lane.
pub fn sample_google_summary_only_visualization_bundle(
) -> Result<RemoteTrainingVisualizationBundle, RemoteTrainingVisualizationError> {
    record_remote_training_visualization_bundle(RemoteTrainingVisualizationBundle {
        schema_version: String::new(),
        bundle_id: String::from("psion-google-summary-only-sample-v1"),
        provider: RemoteTrainingProvider::GoogleCloud,
        profile_id: String::from("google_a2_ultragpu_1g"),
        lane_id: String::from("psion.google_single_node.accelerated"),
        run_id: String::from("psion-google-summary-only-sample"),
        repo_revision: String::from("main@5e6f5f0f"),
        result_classification: RemoteTrainingResultClassification::CompletedSuccess,
        refresh_contract: RemoteTrainingRefreshContract {
            target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
            emission_mode: RemoteTrainingEmissionMode::PostRunOnly,
            last_heartbeat_at_ms: Some(1_742_760_920_000),
            heartbeat_seq: 1,
        },
        series_status: RemoteTrainingSeriesStatus::Unavailable,
        series_unavailable_reason: Some(String::from(
            "no canonical optimizer-step loss or math series was retained for this completed Google lane",
        )),
        timeline: vec![
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_760_100_000,
                phase: String::from("provisioning"),
                subphase: Some(String::from("boot")),
                detail: String::from(
                    "Google single-node provisioning completed and the output directory was created.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_760_400_000,
                phase: String::from("training"),
                subphase: Some(String::from("accelerated_run")),
                detail: String::from(
                    "The accelerated single-node Psion run produced observability receipts and GPU samples.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_760_920_000,
                phase: String::from("finalize"),
                subphase: Some(String::from("upload")),
                detail: String::from(
                    "The finalizer uploaded the summary artifacts and sealed the completed manifest.",
                ),
            },
        ],
        summary: RemoteTrainingVisualizationSummary {
            total_steps_completed: 2_048,
            latest_global_step: Some(2_048),
            latest_train_loss: None,
            latest_ema_loss: None,
            latest_validation_loss: None,
            latest_tokens_per_second: Some(52_105),
            latest_samples_per_second_milli: Some(12_750),
            accumulated_cost_microusd: Some(18_950_000),
            latest_checkpoint_ref: Some(String::from(
                "gcs://psion-runs/google/single-node/pilot/promoted-checkpoint",
            )),
            detail: String::from(
                "This sample keeps summary, GPU, and provenance truth explicit while refusing to invent a loss curve.",
            ),
        },
        heartbeat_series: Vec::new(),
        loss_series: Vec::new(),
        math_series: Vec::new(),
        runtime_series: Vec::new(),
        gpu_series: vec![
            RemoteTrainingGpuSample {
                observed_at_ms: 1_742_760_450_000,
                device_id: String::from("cuda:0"),
                device_label: String::from("NVIDIA A100 80GB"),
                utilization_bps: 8_820,
                memory_used_bytes: 57 * 1024 * 1024 * 1024,
                memory_total_bytes: 80 * 1024 * 1024 * 1024,
                temperature_celsius: Some(63),
                power_watts: Some(286),
            },
            RemoteTrainingGpuSample {
                observed_at_ms: 1_742_760_600_000,
                device_id: String::from("cuda:0"),
                device_label: String::from("NVIDIA A100 80GB"),
                utilization_bps: 8_940,
                memory_used_bytes: 58 * 1024 * 1024 * 1024,
                memory_total_bytes: 80 * 1024 * 1024 * 1024,
                temperature_celsius: Some(64),
                power_watts: Some(291),
            },
            RemoteTrainingGpuSample {
                observed_at_ms: 1_742_760_750_000,
                device_id: String::from("cuda:0"),
                device_label: String::from("NVIDIA A100 80GB"),
                utilization_bps: 8_730,
                memory_used_bytes: 58 * 1024 * 1024 * 1024 + 536_870_912,
                memory_total_bytes: 80 * 1024 * 1024 * 1024,
                temperature_celsius: Some(64),
                power_watts: Some(289),
            },
        ],
        distributed_series: Vec::new(),
        event_series: vec![
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_760_430_000,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("gpu_samples_uploaded"),
                detail: String::from(
                    "The Google lane retained GPU samples in the output directory.",
                ),
            },
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_760_920_000,
                severity: RemoteTrainingEventSeverity::Warning,
                event_kind: String::from("series_unavailable"),
                detail: String::from(
                    "No optimizer-step loss or math series was retained for this completed Google lane.",
                ),
            },
        ],
        source_artifacts: vec![
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("google_final_manifest"),
                artifact_uri: String::from(
                    "gcs://psion-runs/google/single-node/pilot/psion_google_run_final_manifest.json",
                ),
                artifact_digest: Some(String::from(
                    "6763f155f1135f54a90a250b3391edfc73f1be44dbe3cf7a2122a5f06d39fc8f",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::FinalizerOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from("psion.google_run_final_manifest.v1")],
                detail: String::from(
                    "The final manifest remains authoritative for completed Google run identity and provenance.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("observability_receipt"),
                artifact_uri: String::from(
                    "fixtures/psion/observability/psion_pilot_pretrain_run_observability_receipt_v1.json",
                ),
                artifact_digest: Some(String::from(
                    "e4b3d955a9ce0f47db2f45dfd8d94d05c54c2f1ba1ad1b595f281a0d35f7ab5a",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psion.pretrain_run_observability_receipt.v1",
                )],
                detail: String::from(
                    "The observability receipt remains authoritative for cost and throughput summary facts.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("gpu_samples"),
                artifact_uri: String::from(
                    "gcs://psion-runs/google/single-node/pilot/psion_google_gpu_samples.csv",
                ),
                artifact_digest: Some(String::from(
                    "88df41e4b67d246d088cc661c7b04588f1c955d02252be97e53d2553fc5980d1",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::ProviderGenerated,
                authoritative: true,
                source_receipt_ids: Vec::new(),
                detail: String::from(
                    "GPU samples stay authoritative for device activity even when the lane lacks a loss series.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("local_mirror_bundle"),
                artifact_uri: String::from(
                    "local-mirror://psion/google-single-node/remote_training_visualization_bundle.json",
                ),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
                authoritative: false,
                source_receipt_ids: Vec::new(),
                detail: String::from(
                    "The app should prefer the normalized local mirror over raw provider objects when one exists.",
                ),
            },
        ],
        bundle_digest: String::new(),
    })
}

/// Returns a canonical always-live sample bundle for the accelerated Google
/// single-node Psion lane.
pub fn sample_google_live_visualization_bundle(
) -> Result<RemoteTrainingVisualizationBundle, RemoteTrainingVisualizationError> {
    record_remote_training_visualization_bundle(RemoteTrainingVisualizationBundle {
        schema_version: String::new(),
        bundle_id: String::from("psion-google-live-sample-v1"),
        provider: RemoteTrainingProvider::GoogleCloud,
        profile_id: String::from("g2_l4_single_node_accelerated"),
        lane_id: String::from("psion_accelerated_reference_pilot"),
        run_id: String::from("psion-google-live-sample"),
        repo_revision: String::from("main@979af617"),
        result_classification: RemoteTrainingResultClassification::Active,
        refresh_contract: RemoteTrainingRefreshContract {
            target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
            emission_mode: RemoteTrainingEmissionMode::AppendOnlySnapshots,
            last_heartbeat_at_ms: Some(1_742_846_404_900),
            heartbeat_seq: 4,
        },
        series_status: RemoteTrainingSeriesStatus::Available,
        series_unavailable_reason: None,
        timeline: vec![
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_400_000,
                phase: String::from("dataset_staging"),
                subphase: Some(String::from("reference_corpus_build")),
                detail: String::from(
                    "The accelerated Google single-node Psion lane materialized the repo-owned reference corpus.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_401_000,
                phase: String::from("evaluation"),
                subphase: Some(String::from("initial_validation")),
                detail: String::from(
                    "The accelerated lane scored its initial validation posture before CUDA optimizer steps.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_402_000,
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                detail: String::from(
                    "The accelerated lane entered measured CUDA optimizer steps and began emitting one-second heartbeats.",
                ),
            },
        ],
        summary: RemoteTrainingVisualizationSummary {
            total_steps_completed: 2,
            latest_global_step: Some(2),
            latest_train_loss: Some(2.182),
            latest_ema_loss: None,
            latest_validation_loss: Some(2.041),
            latest_tokens_per_second: Some(51_200),
            latest_samples_per_second_milli: Some(8_000),
            accumulated_cost_microusd: None,
            latest_checkpoint_ref: Some(String::from("checkpoint://psion/reference_pilot/step-2")),
            detail: String::from(
                "The accelerated Google single-node Psion lane is active and retaining live loss, math, runtime, and GPU telemetry every second.",
            ),
        },
        heartbeat_series: vec![
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_401_000,
                phase: String::from("evaluation"),
                subphase: Some(String::from("initial_validation")),
                step_in_progress: None,
                microbatch_in_progress: None,
                active_subsystems: vec![String::from("validation"), String::from("held_out_eval")],
                stale_after_ms: 2_500,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_402_000,
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                step_in_progress: Some(1),
                microbatch_in_progress: None,
                active_subsystems: vec![
                    String::from("cuda_graph"),
                    String::from("gradient_batch"),
                    String::from("optimizer_step"),
                ],
                stale_after_ms: 2_500,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_403_000,
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                step_in_progress: Some(2),
                microbatch_in_progress: None,
                active_subsystems: vec![
                    String::from("cuda_graph"),
                    String::from("gradient_batch"),
                    String::from("optimizer_step"),
                ],
                stale_after_ms: 2_500,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_404_900,
                phase: String::from("checkpointing"),
                subphase: Some(String::from("export_checkpoint")),
                step_in_progress: None,
                microbatch_in_progress: None,
                active_subsystems: vec![
                    String::from("checkpoint_export"),
                    String::from("optimizer_state"),
                ],
                stale_after_ms: 2_500,
            },
        ],
        loss_series: vec![
            RemoteTrainingLossSample {
                global_step: Some(0),
                elapsed_ms: 1_000,
                train_loss: None,
                ema_loss: None,
                validation_loss: Some(2.411),
            },
            RemoteTrainingLossSample {
                global_step: Some(1),
                elapsed_ms: 2_000,
                train_loss: Some(2.304),
                ema_loss: None,
                validation_loss: None,
            },
            RemoteTrainingLossSample {
                global_step: Some(2),
                elapsed_ms: 3_000,
                train_loss: Some(2.182),
                ema_loss: None,
                validation_loss: Some(2.041),
            },
        ],
        math_series: vec![
            RemoteTrainingMathSample {
                observed_at_ms: 1_742_846_402_000,
                global_step: Some(1),
                learning_rate: Some(0.0005),
                gradient_norm: Some(1.42),
                parameter_norm: Some(9.81),
                update_norm: Some(0.22),
                clip_fraction: Some(0.21),
                clip_event_count: Some(3),
                loss_scale: None,
                non_finite_count: 0,
                model_specific_diagnostics: BTreeMap::from([
                    (String::from("model_materialization_ms"), 41.0),
                    (String::from("parameter_group_count"), 3.0),
                ]),
            },
            RemoteTrainingMathSample {
                observed_at_ms: 1_742_846_403_000,
                global_step: Some(2),
                learning_rate: Some(0.0005),
                gradient_norm: Some(1.21),
                parameter_norm: Some(9.92),
                update_norm: Some(0.18),
                clip_fraction: Some(0.18),
                clip_event_count: Some(3),
                loss_scale: None,
                non_finite_count: 0,
                model_specific_diagnostics: BTreeMap::from([
                    (String::from("model_materialization_ms"), 39.0),
                    (String::from("parameter_group_count"), 3.0),
                ]),
            },
        ],
        runtime_series: vec![
            RemoteTrainingRuntimeSample {
                observed_at_ms: 1_742_846_402_000,
                data_wait_ms: None,
                forward_ms: Some(811),
                backward_ms: None,
                optimizer_ms: Some(201),
                checkpoint_ms: None,
                evaluation_ms: None,
                tokens_per_second: Some(50_200),
                samples_per_second_milli: Some(7_800),
            },
            RemoteTrainingRuntimeSample {
                observed_at_ms: 1_742_846_403_000,
                data_wait_ms: None,
                forward_ms: Some(789),
                backward_ms: None,
                optimizer_ms: Some(198),
                checkpoint_ms: None,
                evaluation_ms: None,
                tokens_per_second: Some(51_200),
                samples_per_second_milli: Some(8_000),
            },
            RemoteTrainingRuntimeSample {
                observed_at_ms: 1_742_846_404_900,
                data_wait_ms: None,
                forward_ms: None,
                backward_ms: None,
                optimizer_ms: None,
                checkpoint_ms: Some(144),
                evaluation_ms: Some(320),
                tokens_per_second: None,
                samples_per_second_milli: None,
            },
        ],
        gpu_series: vec![
            RemoteTrainingGpuSample {
                observed_at_ms: 1_742_846_402_000,
                device_id: String::from("cuda:0"),
                device_label: String::from("NVIDIA L4"),
                utilization_bps: 8_400,
                memory_used_bytes: 11 * 1024 * 1024 * 1024,
                memory_total_bytes: 24 * 1024 * 1024 * 1024,
                temperature_celsius: Some(58),
                power_watts: Some(152),
            },
            RemoteTrainingGpuSample {
                observed_at_ms: 1_742_846_403_000,
                device_id: String::from("cuda:0"),
                device_label: String::from("NVIDIA L4"),
                utilization_bps: 8_650,
                memory_used_bytes: 11 * 1024 * 1024 * 1024 + 268_435_456,
                memory_total_bytes: 24 * 1024 * 1024 * 1024,
                temperature_celsius: Some(59),
                power_watts: Some(156),
            },
        ],
        distributed_series: Vec::new(),
        event_series: vec![
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_846_400_000,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("trainer_started"),
                detail: String::from(
                    "The accelerated Google single-node lane created its provider-neutral live visualization bundle.",
                ),
            },
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_846_404_900,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("checkpoint_written"),
                detail: String::from(
                    "The accelerated lane retained its promoted checkpoint ref and checkpoint timing in the live bundle.",
                ),
            },
        ],
        source_artifacts: vec![
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("live_bundle"),
                artifact_uri: String::from(
                    "training_visualization/psion_google_single_node_remote_training_visualization_bundle_v1.json",
                ),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psion.google_single_node.remote_training_live_bundle.v1",
                )],
                detail: String::from(
                    "The provider-neutral local mirror is the app-facing source for the accelerated Google lane.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("stage_receipt"),
                artifact_uri: String::from("psion_accelerated_reference_pilot_stage_receipt.json"),
                artifact_digest: Some(String::from(
                    "14304ef8c38407f9f057f7c2665a02de862c7f3fd1d70b330ebc0f245f88d96e",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from("receipt.psion.pretrain_stage.v1")],
                detail: String::from(
                    "The accelerated stage receipt remains authoritative for delivered execution and accelerator posture.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("observability_receipt"),
                artifact_uri: String::from(
                    "psion_accelerated_reference_pilot_observability_receipt.json",
                ),
                artifact_digest: Some(String::from(
                    "d4cc8c23d2676cb7c8f2cda42c574bb99d1da69853672f159f5130cffcb2d9b9",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psion.pretrain_run_observability_receipt.v1",
                )],
                detail: String::from(
                    "The observability receipt remains authoritative for throughput and topology summary facts.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("checkpoint_manifest"),
                artifact_uri: String::from(
                    "psion_accelerated_reference_pilot_checkpoint_manifest.json",
                ),
                artifact_digest: Some(String::from(
                    "51b4e6e91f5f09662cb4481933d2924f9cdf730b071b607d5bbf39919c7d5dd8",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "psion_reference_pilot_checkpoint_manifest.v1",
                )],
                detail: String::from(
                    "The checkpoint manifest remains authoritative for the promoted checkpoint identity surfaced in the live viewer.",
                ),
            },
        ],
        bundle_digest: String::new(),
    })
}

/// Returns a canonical RunPod distributed `8xH100` sample bundle that proves
/// the lane can stay visibly live every second while it retains distributed
/// step, validation, GPU, and provenance truth.
pub fn sample_parameter_golf_distributed_live_visualization_bundle(
) -> Result<RemoteTrainingVisualizationBundle, RemoteTrainingVisualizationError> {
    record_remote_training_visualization_bundle(RemoteTrainingVisualizationBundle {
        schema_version: String::new(),
        bundle_id: String::from("parameter-golf-runpod-distributed-8xh100-sample-v1"),
        provider: RemoteTrainingProvider::RunPod,
        profile_id: String::from("runpod_8xh100_parameter_golf"),
        lane_id: String::from("parameter_golf_distributed_8xh100"),
        run_id: String::from("parameter-golf-runpod-8xh100-sample"),
        repo_revision: String::from("main@5e6f5f0f"),
        result_classification: RemoteTrainingResultClassification::Active,
        refresh_contract: RemoteTrainingRefreshContract {
            target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
            emission_mode: RemoteTrainingEmissionMode::AppendOnlySnapshots,
            last_heartbeat_at_ms: Some(1_742_846_512_000),
            heartbeat_seq: 3,
        },
        series_status: RemoteTrainingSeriesStatus::Available,
        series_unavailable_reason: None,
        timeline: vec![
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_509_000,
                phase: String::from("training"),
                subphase: Some(String::from("runtime_bootstrap")),
                detail: String::from(
                    "The RunPod 8xH100 runtime started and created its provider-neutral live visualization mirror.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_510_500,
                phase: String::from("training"),
                subphase: Some(String::from("distributed_train_step")),
                detail: String::from(
                    "The distributed runtime advanced from bootstrap into the first optimizer step.",
                ),
            },
            RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_512_000,
                phase: String::from("evaluation"),
                subphase: Some(String::from("distributed_validation")),
                detail: String::from(
                    "The distributed runtime retained the first measured train-step sample and is now sharding sliding-window validation across all eight ranks.",
                ),
            },
        ],
        summary: RemoteTrainingVisualizationSummary {
            total_steps_completed: 1,
            latest_global_step: Some(1),
            latest_train_loss: Some(8.288431),
            latest_ema_loss: None,
            latest_validation_loss: Some(4.971701),
            latest_tokens_per_second: Some(618_475),
            latest_samples_per_second_milli: None,
            accumulated_cost_microusd: None,
            latest_checkpoint_ref: Some(String::from(
                "parameter-golf-distributed-8xh100-run/benchmark/runtime_model_artifacts/post_step_1_final_model.int8.ptz",
            )),
            detail: String::from(
                "The RunPod 8xH100 lane is emitting one-second heartbeats while retaining distributed step, validation, GPU, and provenance telemetry in one provider-neutral bundle.",
            ),
        },
        heartbeat_series: vec![
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_510_000,
                phase: String::from("training"),
                subphase: Some(String::from("runtime_bootstrap")),
                step_in_progress: Some(1),
                microbatch_in_progress: None,
                active_subsystems: vec![
                    String::from("runtime_bootstrap"),
                    String::from("rank_fanout"),
                    String::from("distributed_runtime"),
                ],
                stale_after_ms: 2_500,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_511_000,
                phase: String::from("training"),
                subphase: Some(String::from("distributed_train_step")),
                step_in_progress: Some(1),
                microbatch_in_progress: None,
                active_subsystems: vec![
                    String::from("distributed_train_step"),
                    String::from("rank_wait"),
                    String::from("cuda_execution"),
                ],
                stale_after_ms: 2_500,
            },
            RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_512_000,
                phase: String::from("evaluation"),
                subphase: Some(String::from("distributed_validation")),
                step_in_progress: Some(1),
                microbatch_in_progress: None,
                active_subsystems: vec![
                    String::from("distributed_validation"),
                    String::from("rank_wait"),
                    String::from("cuda_evaluation"),
                ],
                stale_after_ms: 2_500,
            },
        ],
        loss_series: vec![RemoteTrainingLossSample {
            global_step: Some(1),
            elapsed_ms: 2_000,
            train_loss: Some(8.288431),
            ema_loss: None,
            validation_loss: Some(4.971701),
        }],
        math_series: vec![RemoteTrainingMathSample {
            observed_at_ms: 1_742_846_511_750,
            global_step: Some(1),
            learning_rate: None,
            gradient_norm: Some(1.8821),
            parameter_norm: None,
            update_norm: None,
            clip_fraction: None,
            clip_event_count: Some(1),
            loss_scale: None,
            non_finite_count: 0,
            model_specific_diagnostics: BTreeMap::from([
                (String::from("gradient_sync_ms"), 3_612.0),
                (String::from("optimizer_step_ms"), 923.0),
                (String::from("validation_sequence_count"), 121_152.0),
            ]),
        }],
        runtime_series: vec![RemoteTrainingRuntimeSample {
            observed_at_ms: 1_742_846_511_750,
            data_wait_ms: None,
            forward_ms: Some(98_659),
            backward_ms: Some(128_153),
            optimizer_ms: Some(923),
            checkpoint_ms: None,
            evaluation_ms: Some(1472161),
            tokens_per_second: Some(618_475),
            samples_per_second_milli: None,
        }],
        gpu_series: (0_u32..8)
            .map(|device_index| RemoteTrainingGpuSample {
                observed_at_ms: 1_742_846_511_500 + u64::from(device_index) * 25,
                device_id: format!("cuda:{device_index}"),
                device_label: String::from("NVIDIA H100 80GB HBM3"),
                utilization_bps: 7_600 + device_index * 20,
                memory_used_bytes: (13 + u64::from(device_index)) * 1024 * 1024 * 1024,
                memory_total_bytes: 80 * 1024 * 1024 * 1024,
                temperature_celsius: Some(66 + device_index as u16),
                power_watts: Some(286 + device_index as u16),
            })
            .collect(),
        distributed_series: vec![RemoteTrainingDistributedSample {
            observed_at_ms: 1_742_846_511_750,
            participating_rank_count: 8,
            rank_skew_ms: Some(11_742),
            slowest_rank_ms: Some(229_019),
            collective_ms: Some(3_612),
            stalled_rank_count: 0,
        }],
        event_series: vec![
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_846_509_000,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("runtime_started"),
                detail: String::from(
                    "The RunPod distributed runtime created its provider-neutral live visualization bundle.",
                ),
            },
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_846_511_750,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("train_step_recorded"),
                detail: String::from(
                    "The lane retained its first measured train-step sample and distributed validation aggregation.",
                ),
            },
            RemoteTrainingEventSample {
                observed_at_ms: 1_742_846_512_000,
                severity: RemoteTrainingEventSeverity::Info,
                event_kind: String::from("distributed_validation_active"),
                detail: String::from(
                    "Sliding-window validation is active across all eight ranks and the bundle is staying fresh every second.",
                ),
            },
        ],
        source_artifacts: vec![
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("live_bundle"),
                artifact_uri: String::from(
                    "training_visualization/parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v1.json",
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
                artifact_role: String::from("bringup_report"),
                artifact_uri: String::from(
                    "exported_submission/records/track_non_record_16mb/2026-03-18_psionic_local_reference_runtime_replay_v2/parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_bringup.json",
                ),
                artifact_digest: Some(String::from(
                    "43dfca6d9bf092fb9cc8bd44fd719be8d2f19691f3ba790902b3d73fdb070f74",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf_distributed_8xh100_bringup_report",
                )],
                detail: String::from(
                    "The bring-up report remains authoritative for exact machine admission on the distributed lane.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("gpu_inventory"),
                artifact_uri: String::from("nvidia_smi_inventory.txt"),
                artifact_digest: Some(String::from(
                    "6f5ea71f3769b8db836099112654587d8b7885bbc26b2f0e63de6155772e55a8",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::ProviderGenerated,
                authoritative: true,
                source_receipt_ids: Vec::new(),
                detail: String::from(
                    "The `nvidia-smi` inventory snapshot remains authoritative for the retained per-device GPU state.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("gpu_topology"),
                artifact_uri: String::from("nvidia_smi_topology.txt"),
                artifact_digest: Some(String::from(
                    "f7c7f6c5d41468deeb6f9880b95dd8ed8d57ae00b6ae3a1e8143550bd1ea7a84",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::ProviderGenerated,
                authoritative: true,
                source_receipt_ids: Vec::new(),
                detail: String::from(
                    "The `nvidia-smi topo -m` capture remains authoritative for the retained fabric topology snapshot.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("runtime_manifest"),
                artifact_uri: String::from("runtime/runtime_manifest.json"),
                artifact_digest: Some(String::from(
                    "54b0f4e53df5f3923d2256ca1a92cdd78d06f9a92a07b0bc037d698f2790ad5f",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from("parameter_golf_submission_runtime_manifest")],
                detail: String::from(
                    "The shipped runtime manifest remains authoritative for the exported-folder runtime contract.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("train_step_receipt"),
                artifact_uri: String::from(
                    "exported_submission/records/track_non_record_16mb/2026-03-18_psionic_local_reference_runtime_replay_v2/parameter-golf-distributed-8xh100-run/benchmark/parameter_golf_distributed_8xh100_train_step.json",
                ),
                artifact_digest: Some(String::from(
                    "365d50fb99f74154d17f5fb5148cb85e353e03b13e6007ab7ecfd46c77d220de",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf_distributed_8xh100_train_step_receipt",
                )],
                detail: String::from(
                    "The train-step receipt remains authoritative for the retained distributed step, validation, and artifact lineage.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("current_model_artifact"),
                artifact_uri: String::from(
                    "exported_submission/records/track_non_record_16mb/2026-03-18_psionic_local_reference_runtime_replay_v2/parameter-golf-distributed-8xh100-run/benchmark/runtime_model_artifacts/post_step_1_final_model.int8.ptz",
                ),
                artifact_digest: Some(String::from(
                    "4657d793ae3e64796670b6768f433c48f788518725ba2854e913db205412b250",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf_distributed_8xh100_train_step_receipt",
                )],
                detail: String::from(
                    "The retained post-step int8+zlib artifact remains authoritative for the current distributed runtime output.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("submission_run_evidence"),
                artifact_uri: String::from(
                    "exported_submission/psionic_parameter_golf_submission_run_evidence.json",
                ),
                artifact_digest: Some(String::from(
                    "f65656fd3747ab65da841b42cfc9aa7433416ee6680b99f000b3ca4ab622f23f",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from("parameter_golf.submission_run_evidence.v1")],
                detail: String::from(
                    "The submission run evidence report remains authoritative for the exported-folder replay posture and embedded distributed challenge receipt.",
                ),
            },
            RemoteTrainingSourceArtifact {
                artifact_role: String::from("distributed_receipt"),
                artifact_uri: String::from("parameter_golf_distributed_8xh100_receipt.json"),
                artifact_digest: Some(String::from(
                    "5e019c3a263a0e3adf0f09d1050c59dfbf4e0ad7c8270af7e664aad7faded21b",
                )),
                source_kind: RemoteTrainingArtifactSourceKind::RuntimeOwned,
                authoritative: true,
                source_receipt_ids: vec![String::from(
                    "parameter_golf_distributed_throughput_receipt",
                )],
                detail: String::from(
                    "The distributed challenge receipt remains authoritative for refusal posture and any retained topology or timing facts.",
                ),
            },
        ],
        bundle_digest: String::new(),
    })
}

/// Backward-compatible alias for earlier code paths that still refer to the
/// distributed lane as unavailable-only. The canonical fixture is now the live
/// sample above.
pub fn sample_parameter_golf_distributed_unavailable_visualization_bundle(
) -> Result<RemoteTrainingVisualizationBundle, RemoteTrainingVisualizationError> {
    sample_parameter_golf_distributed_live_visualization_bundle()
}

/// Returns a canonical run index that enumerates both a full-series lane and a
/// summary-only lane with the same top-level shape.
pub fn sample_remote_training_run_index(
) -> Result<RemoteTrainingRunIndex, RemoteTrainingVisualizationError> {
    let google = sample_google_summary_only_visualization_bundle()?;
    let parameter_golf = sample_parameter_golf_live_visualization_bundle()?;
    let parameter_golf_distributed = sample_parameter_golf_distributed_live_visualization_bundle()?;
    build_remote_training_run_index(RemoteTrainingRunIndex {
        schema_version: String::new(),
        index_id: String::from("remote-training-run-index-sample-v1"),
        generated_at_ms: 1_742_846_405_000,
        entries: vec![
            RemoteTrainingRunIndexEntry {
                provider: google.provider,
                profile_id: google.profile_id.clone(),
                lane_id: google.lane_id.clone(),
                run_id: google.run_id.clone(),
                repo_revision: google.repo_revision.clone(),
                result_classification: google.result_classification,
                series_status: google.series_status,
                series_unavailable_reason: google.series_unavailable_reason.clone(),
                last_heartbeat_at_ms: google.refresh_contract.last_heartbeat_at_ms,
                bundle_artifact_uri: Some(String::from(
                    "fixtures/training_visualization/psion_google_summary_only_remote_training_visualization_bundle_v1.json",
                )),
                bundle_digest: Some(google.bundle_digest.clone()),
                summary_label: String::from("Google single-node Psion summary-only sample"),
                detail: String::from(
                    "This entry proves the index can list a summary-only lane without pretending it has a loss curve.",
                ),
            },
            RemoteTrainingRunIndexEntry {
                provider: parameter_golf.provider,
                profile_id: parameter_golf.profile_id.clone(),
                lane_id: parameter_golf.lane_id.clone(),
                run_id: parameter_golf.run_id.clone(),
                repo_revision: parameter_golf.repo_revision.clone(),
                result_classification: parameter_golf.result_classification,
                series_status: parameter_golf.series_status,
                series_unavailable_reason: parameter_golf.series_unavailable_reason.clone(),
                last_heartbeat_at_ms: parameter_golf.refresh_contract.last_heartbeat_at_ms,
                bundle_artifact_uri: Some(String::from(
                    "fixtures/training_visualization/parameter_golf_live_remote_training_visualization_bundle_v1.json",
                )),
                bundle_digest: Some(parameter_golf.bundle_digest.clone()),
                summary_label: String::from("RunPod single-H100 PGOLF live sample"),
                detail: String::from(
                    "This entry proves the same index can list a full always-live lane with optimizer-step telemetry.",
                ),
            },
            RemoteTrainingRunIndexEntry {
                provider: parameter_golf_distributed.provider,
                profile_id: parameter_golf_distributed.profile_id.clone(),
                lane_id: parameter_golf_distributed.lane_id.clone(),
                run_id: parameter_golf_distributed.run_id.clone(),
                repo_revision: parameter_golf_distributed.repo_revision.clone(),
                result_classification: parameter_golf_distributed.result_classification,
                series_status: parameter_golf_distributed.series_status,
                series_unavailable_reason: parameter_golf_distributed
                    .series_unavailable_reason
                    .clone(),
                last_heartbeat_at_ms: parameter_golf_distributed
                    .refresh_contract
                    .last_heartbeat_at_ms,
                bundle_artifact_uri: Some(String::from(
                    "fixtures/training_visualization/parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v1.json",
                )),
                bundle_digest: Some(parameter_golf_distributed.bundle_digest.clone()),
                summary_label: String::from("RunPod 8xH100 PGOLF distributed live sample"),
                detail: String::from(
                    "This entry proves the same index can list an always-live distributed lane that retains heartbeats, distributed step telemetry, and provenance in one provider-neutral bundle.",
                ),
            },
        ],
        detail: String::from(
            "The run index keeps summary-only, always-live single-node, and always-live distributed lanes in one provider-neutral discovery surface without forcing the app to walk provider roots.",
        ),
        index_digest: String::new(),
    })
}

fn stable_bundle_digest(bundle: &RemoteTrainingVisualizationBundle) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    stable_digest(b"remote_training_visualization_bundle|", &canonical)
}

fn stable_run_index_digest(index: &RemoteTrainingRunIndex) -> String {
    let mut canonical = index.clone();
    canonical.index_digest.clear();
    stable_digest(b"remote_training_run_index|", &canonical)
}

fn stable_digest<T>(prefix: &[u8], value: &T) -> String
where
    T: Serialize,
{
    let encoded = match serde_json::to_vec(value) {
        Ok(encoded) => encoded,
        Err(error) => error.to_string().into_bytes(),
    };
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(encoded);
    hex::encode(hasher.finalize())
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), RemoteTrainingVisualizationError> {
    if value.trim().is_empty() {
        return Err(RemoteTrainingVisualizationError::MissingField {
            field: String::from(field),
        });
    }
    Ok(())
}

fn validate_optional_nonempty(
    value: Option<&str>,
    field: &str,
) -> Result<(), RemoteTrainingVisualizationError> {
    if let Some(value) = value {
        ensure_nonempty(value, field)?;
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), RemoteTrainingVisualizationError> {
    ensure_nonempty(actual, field)?;
    if actual != expected {
        return Err(RemoteTrainingVisualizationError::FieldMismatch {
            field: String::from(field),
            expected: String::from(expected),
            actual: String::from(actual),
        });
    }
    Ok(())
}

fn reject_duplicate_strings(
    values: &[String],
    field: &str,
) -> Result<(), RemoteTrainingVisualizationError> {
    let mut seen = BTreeSet::new();
    for value in values {
        ensure_nonempty(value.as_str(), field)?;
        if !seen.insert(value.as_str()) {
            return Err(RemoteTrainingVisualizationError::DuplicateValue {
                field: String::from(field),
                value: value.clone(),
            });
        }
    }
    Ok(())
}

fn validate_metric(value: f32, field: &str) -> Result<(), RemoteTrainingVisualizationError> {
    if !value.is_finite() || value < 0.0 {
        return Err(RemoteTrainingVisualizationError::InvalidValue {
            field: String::from(field),
            detail: String::from("metric must be finite and non-negative"),
        });
    }
    Ok(())
}

fn validate_optional_metric(
    value: Option<f32>,
    field: &str,
) -> Result<(), RemoteTrainingVisualizationError> {
    if let Some(value) = value {
        validate_metric(value, field)?;
    }
    Ok(())
}

fn validate_optional_fraction(
    value: Option<f32>,
    field: &str,
) -> Result<(), RemoteTrainingVisualizationError> {
    if let Some(value) = value {
        validate_metric(value, field)?;
        if value > 1.0 {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: String::from(field),
                detail: String::from("fraction must stay inside 0.0..=1.0"),
            });
        }
    }
    Ok(())
}

fn validate_optional_duration(
    value: Option<u64>,
    field: &str,
) -> Result<(), RemoteTrainingVisualizationError> {
    if value == Some(0) {
        return Err(RemoteTrainingVisualizationError::InvalidValue {
            field: String::from(field),
            detail: String::from("duration must be positive when present"),
        });
    }
    Ok(())
}

fn validate_monotonic_timestamps<I>(
    values: I,
    field: &str,
) -> Result<(), RemoteTrainingVisualizationError>
where
    I: IntoIterator<Item = u64>,
{
    let mut previous = None;
    for value in values {
        if let Some(previous) = previous {
            if value < previous {
                return Err(RemoteTrainingVisualizationError::InvalidValue {
                    field: String::from(field),
                    detail: String::from("timestamps must be monotonic"),
                });
            }
        }
        previous = Some(value);
    }
    Ok(())
}

fn validate_monotonic_timeline(
    timeline: &[RemoteTrainingTimelineEntry],
) -> Result<(), RemoteTrainingVisualizationError> {
    if timeline.is_empty() {
        return Err(RemoteTrainingVisualizationError::MissingField {
            field: String::from("remote_training_visualization_bundle.timeline"),
        });
    }
    validate_monotonic_timestamps(
        timeline.iter().map(|entry| entry.observed_at_ms),
        "remote_training_visualization_bundle.timeline.observed_at_ms",
    )
}

/// Validation failures for the remote-training visualization contract.
#[derive(Debug, Error)]
pub enum RemoteTrainingVisualizationError {
    #[error("remote training visualization is missing field `{field}`")]
    MissingField { field: String },
    #[error(
        "remote training visualization field `{field}` expected `{expected}` but found `{actual}`"
    )]
    FieldMismatch {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("remote training visualization field `{field}` is invalid: {detail}")]
    InvalidValue { field: String, detail: String },
    #[error("remote training visualization field `{field}` contains duplicate value `{value}`")]
    DuplicateValue { field: String, value: String },
    #[error("remote training visualization digest mismatch for `{kind}`")]
    DigestMismatch { kind: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_google_bundle() -> RemoteTrainingVisualizationBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/psion_google_summary_only_remote_training_visualization_bundle_v1.json"
        ))
        .expect("summary-only sample bundle should parse")
    }

    fn sample_parameter_golf_bundle() -> RemoteTrainingVisualizationBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/parameter_golf_live_remote_training_visualization_bundle_v1.json"
        ))
        .expect("live sample bundle should parse")
    }

    fn sample_google_live_bundle() -> RemoteTrainingVisualizationBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/psion_google_live_remote_training_visualization_bundle_v1.json"
        ))
        .expect("google live sample bundle should parse")
    }

    fn sample_parameter_golf_distributed_bundle() -> RemoteTrainingVisualizationBundle {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v1.json"
        ))
        .expect("distributed sample bundle should parse")
    }

    fn sample_index() -> RemoteTrainingRunIndex {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/remote_training_run_index_v1.json"
        ))
        .expect("run index should parse")
    }

    #[test]
    fn summary_only_sample_bundle_stays_valid() -> Result<(), RemoteTrainingVisualizationError> {
        let bundle = sample_google_bundle();
        bundle.validate()
    }

    #[test]
    fn live_sample_bundle_stays_valid() -> Result<(), RemoteTrainingVisualizationError> {
        let bundle = sample_parameter_golf_bundle();
        bundle.validate()
    }

    #[test]
    fn google_live_sample_bundle_stays_valid() -> Result<(), RemoteTrainingVisualizationError> {
        let bundle = sample_google_live_bundle();
        bundle.validate()
    }

    #[test]
    fn distributed_sample_bundle_stays_valid() -> Result<(), RemoteTrainingVisualizationError> {
        let bundle = sample_parameter_golf_distributed_bundle();
        bundle.validate()
    }

    #[test]
    fn sample_run_index_stays_valid() -> Result<(), RemoteTrainingVisualizationError> {
        let index = sample_index();
        index.validate()
    }

    #[test]
    fn active_runs_reject_post_run_only_emission() {
        let bundle = RemoteTrainingVisualizationBundle {
            result_classification: RemoteTrainingResultClassification::Active,
            refresh_contract: RemoteTrainingRefreshContract {
                target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
                emission_mode: RemoteTrainingEmissionMode::PostRunOnly,
                last_heartbeat_at_ms: Some(1),
                heartbeat_seq: 1,
            },
            ..sample_parameter_golf_live_bundle_struct()
        };
        let error = bundle.finalize().expect_err("bundle should fail");
        assert!(matches!(
            error,
            RemoteTrainingVisualizationError::InvalidValue { field, .. }
                if field == "remote_training_visualization_bundle.refresh_contract.emission_mode"
        ));
    }

    #[test]
    fn available_series_require_loss_samples() {
        let mut bundle = sample_parameter_golf_live_bundle_struct();
        bundle.loss_series.clear();
        let error = bundle.finalize().expect_err("bundle should fail");
        assert!(matches!(
            error,
            RemoteTrainingVisualizationError::InvalidValue { field, .. }
                if field == "remote_training_visualization_bundle.loss_series"
        ));
    }

    #[test]
    fn run_index_requires_unavailable_reason_for_summary_only_lane() {
        let mut index = sample_remote_training_run_index_struct();
        index.entries[0].series_unavailable_reason = None;
        let error = index.finalize().expect_err("index should fail");
        assert!(matches!(
            error,
            RemoteTrainingVisualizationError::MissingField { field }
                if field == "remote_training_run_index.entries.series_unavailable_reason"
        ));
    }

    fn sample_parameter_golf_live_bundle_struct() -> RemoteTrainingVisualizationBundle {
        RemoteTrainingVisualizationBundle {
            schema_version: String::new(),
            bundle_id: String::from("parameter-golf-runpod-single-h100-live-sample-v1"),
            provider: RemoteTrainingProvider::RunPod,
            profile_id: String::from("runpod_h100_single_gpu"),
            lane_id: String::from("parameter_golf.runpod_single_h100"),
            run_id: String::from("parameter-golf-runpod-single-h100-live-sample"),
            repo_revision: String::from("main@5e6f5f0f"),
            result_classification: RemoteTrainingResultClassification::Active,
            refresh_contract: RemoteTrainingRefreshContract {
                target_ui_update_interval_ms: REMOTE_TRAINING_TARGET_UI_UPDATE_INTERVAL_MS,
                emission_mode: RemoteTrainingEmissionMode::AppendOnlySnapshots,
                last_heartbeat_at_ms: Some(1_742_846_402_000),
                heartbeat_seq: 5,
            },
            series_status: RemoteTrainingSeriesStatus::Available,
            series_unavailable_reason: None,
            timeline: vec![RemoteTrainingTimelineEntry {
                observed_at_ms: 1_742_846_395_000,
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                detail: String::from("sample"),
            }],
            summary: RemoteTrainingVisualizationSummary {
                total_steps_completed: 1,
                latest_global_step: Some(1),
                latest_train_loss: Some(4.2),
                latest_ema_loss: Some(4.2),
                latest_validation_loss: None,
                latest_tokens_per_second: Some(100),
                latest_samples_per_second_milli: Some(1),
                accumulated_cost_microusd: Some(1),
                latest_checkpoint_ref: Some(String::from("checkpoint://sample")),
                detail: String::from("sample"),
            },
            heartbeat_series: vec![RemoteTrainingHeartbeatSample {
                observed_at_ms: 1_742_846_402_000,
                phase: String::from("training"),
                subphase: Some(String::from("optimizer_step")),
                step_in_progress: Some(1),
                microbatch_in_progress: Some(1),
                active_subsystems: vec![String::from("forward")],
                stale_after_ms: 2_500,
            }],
            loss_series: vec![RemoteTrainingLossSample {
                global_step: Some(1),
                elapsed_ms: 1_000,
                train_loss: Some(4.2),
                ema_loss: None,
                validation_loss: None,
            }],
            math_series: Vec::new(),
            runtime_series: Vec::new(),
            gpu_series: Vec::new(),
            distributed_series: Vec::new(),
            event_series: Vec::new(),
            source_artifacts: vec![RemoteTrainingSourceArtifact {
                artifact_role: String::from("live_bundle"),
                artifact_uri: String::from("local-mirror://sample"),
                artifact_digest: None,
                source_kind: RemoteTrainingArtifactSourceKind::LocalMirror,
                authoritative: true,
                source_receipt_ids: Vec::new(),
                detail: String::from("sample"),
            }],
            bundle_digest: String::new(),
        }
    }

    fn sample_remote_training_run_index_struct() -> RemoteTrainingRunIndex {
        RemoteTrainingRunIndex {
            schema_version: String::new(),
            index_id: String::from("remote-training-run-index-sample-v1"),
            generated_at_ms: 1_742_846_405_000,
            entries: vec![RemoteTrainingRunIndexEntry {
                provider: RemoteTrainingProvider::GoogleCloud,
                profile_id: String::from("google_a2_ultragpu_1g"),
                lane_id: String::from("psion.google_single_node.accelerated"),
                run_id: String::from("psion-google-summary-only-sample"),
                repo_revision: String::from("main@5e6f5f0f"),
                result_classification: RemoteTrainingResultClassification::CompletedSuccess,
                series_status: RemoteTrainingSeriesStatus::Unavailable,
                series_unavailable_reason: Some(String::from("missing loss series")),
                last_heartbeat_at_ms: Some(1_742_760_920_000),
                bundle_artifact_uri: Some(String::from("fixtures://sample.json")),
                bundle_digest: Some(String::from("digest")),
                summary_label: String::from("sample"),
                detail: String::from("sample"),
            }],
            detail: String::from("sample"),
            index_digest: String::new(),
        }
    }
}
