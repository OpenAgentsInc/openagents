use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    build_parameter_golf_homegolf_run_index_entry_v2,
    build_parameter_golf_homegolf_visualization_bundle_v2,
    build_parameter_golf_xtrain_run_index_entry_v2,
    build_parameter_golf_xtrain_visualization_bundle_v2, sample_google_live_visualization_bundle,
    sample_google_summary_only_visualization_bundle,
    sample_parameter_golf_distributed_live_visualization_bundle,
    sample_parameter_golf_live_visualization_bundle, RemoteTrainingDistributedSample,
    RemoteTrainingEventSample, RemoteTrainingGpuSample, RemoteTrainingHeartbeatSample,
    RemoteTrainingLossSample, RemoteTrainingMathSample, RemoteTrainingProvider,
    RemoteTrainingRefreshContract, RemoteTrainingResultClassification, RemoteTrainingRunIndexEntry,
    RemoteTrainingRuntimeSample, RemoteTrainingSeriesStatus, RemoteTrainingSourceArtifact,
    RemoteTrainingTimelineEntry, RemoteTrainingVisualizationBundle,
    RemoteTrainingVisualizationError, RemoteTrainingVisualizationSummary,
    PARAMETER_GOLF_NON_RECORD_TRACK_ID, PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES,
};

/// Stable schema version for the track-aware provider-neutral remote-training bundle.
pub const REMOTE_TRAINING_VISUALIZATION_BUNDLE_V2_SCHEMA_VERSION: &str =
    "psionic.remote_training_visualization_bundle.v2";
/// Stable schema version for the track-aware provider-neutral remote-training run index.
pub const REMOTE_TRAINING_RUN_INDEX_V2_SCHEMA_VERSION: &str =
    "psionic.remote_training_run_index.v2";
/// Stable HOMEGOLF track identifier reused by the v2 visualization bundle family.
pub const REMOTE_TRAINING_HOMEGOLF_TRACK_ID: &str =
    "parameter_golf.home_cluster_compatible_10min.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingTrackFamilyV2 {
    Psion,
    ParameterGolf,
    Homegolf,
    Xtrain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingExecutionClassV2 {
    SingleNode,
    DenseDistributed,
    HomeClusterMixedDevice,
    BoundedTrainToInfer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingComparabilityClassV2 {
    NotComparable,
    SameTrackComparable,
    PublicBaselineComparable,
    PublicLeaderboardComparable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingProofPostureV2 {
    SummaryOnly,
    RuntimeMeasured,
    ScoreCloseoutMeasured,
    BoundedTrainToInfer,
    Refused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingPublicEquivalenceClassV2 {
    NotApplicable,
    NotPublicEquivalent,
    PublicBaselineComparableOnly,
    PublicLeaderboardEquivalent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingScoreDirectionV2 {
    LowerIsBetter,
    HigherIsBetter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingScoreCloseoutPostureV2 {
    ScoreUnavailable,
    ScoreHeldPendingCloseout,
    ScoreClosedOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteTrainingPromotionGatePostureV2 {
    NotApplicable,
    Held,
    Eligible,
    Promoted,
    Refused,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteTrainingTrackSemanticsV2 {
    pub track_family: RemoteTrainingTrackFamilyV2,
    pub track_id: String,
    pub execution_class: RemoteTrainingExecutionClassV2,
    pub comparability_class: RemoteTrainingComparabilityClassV2,
    pub proof_posture: RemoteTrainingProofPostureV2,
    pub public_equivalence_class: RemoteTrainingPublicEquivalenceClassV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_law_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_cap_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallclock_cap_seconds: Option<u64>,
    pub semantic_summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingPrimaryScoreV2 {
    pub score_metric_id: String,
    pub score_direction: RemoteTrainingScoreDirectionV2,
    pub score_unit: String,
    pub score_value: f64,
    pub score_value_observed_at_ms: u64,
    pub score_summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingScoreDeltaV2 {
    pub reference_id: String,
    pub score_metric_id: String,
    pub reference_score_value: f64,
    pub delta_value: f64,
    pub delta_summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingScoreSurfaceV2 {
    pub score_closeout_posture: RemoteTrainingScoreCloseoutPostureV2,
    pub promotion_gate_posture: RemoteTrainingPromotionGatePostureV2,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub delta_rows: Vec<RemoteTrainingScoreDeltaV2>,
    pub semantic_summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingVisualizationBundleV2 {
    pub schema_version: String,
    pub bundle_id: String,
    pub provider: RemoteTrainingProvider,
    pub profile_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub repo_revision: String,
    pub track_semantics: RemoteTrainingTrackSemanticsV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_score: Option<RemoteTrainingPrimaryScoreV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_surface: Option<RemoteTrainingScoreSurfaceV2>,
    pub result_classification: RemoteTrainingResultClassification,
    pub refresh_contract: RemoteTrainingRefreshContract,
    pub series_status: RemoteTrainingSeriesStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_unavailable_reason: Option<String>,
    pub timeline: Vec<RemoteTrainingTimelineEntry>,
    pub summary: RemoteTrainingVisualizationSummary,
    pub heartbeat_series: Vec<RemoteTrainingHeartbeatSample>,
    pub loss_series: Vec<RemoteTrainingLossSample>,
    pub math_series: Vec<RemoteTrainingMathSample>,
    pub runtime_series: Vec<RemoteTrainingRuntimeSample>,
    pub gpu_series: Vec<RemoteTrainingGpuSample>,
    pub distributed_series: Vec<RemoteTrainingDistributedSample>,
    pub event_series: Vec<RemoteTrainingEventSample>,
    pub source_artifacts: Vec<RemoteTrainingSourceArtifact>,
    pub bundle_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingRunIndexEntryV2 {
    pub provider: RemoteTrainingProvider,
    pub profile_id: String,
    pub lane_id: String,
    pub run_id: String,
    pub repo_revision: String,
    pub track_semantics: RemoteTrainingTrackSemanticsV2,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_score: Option<RemoteTrainingPrimaryScoreV2>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub score_surface: Option<RemoteTrainingScoreSurfaceV2>,
    pub result_classification: RemoteTrainingResultClassification,
    pub series_status: RemoteTrainingSeriesStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub series_unavailable_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_heartbeat_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_artifact_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_digest: Option<String>,
    pub semantic_summary: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RemoteTrainingRunIndexV2 {
    pub schema_version: String,
    pub index_id: String,
    pub generated_at_ms: u64,
    pub entries: Vec<RemoteTrainingRunIndexEntryV2>,
    pub detail: String,
    pub index_digest: String,
}

impl RemoteTrainingTrackSemanticsV2 {
    fn validate(&self, field_prefix: &str) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.track_id.as_str(),
            format!("{field_prefix}.track_id").as_str(),
        )?;
        validate_optional_nonempty(
            self.score_law_ref.as_deref(),
            format!("{field_prefix}.score_law_ref").as_str(),
        )?;
        ensure_nonempty(
            self.semantic_summary.as_str(),
            format!("{field_prefix}.semantic_summary").as_str(),
        )?;
        if self.artifact_cap_bytes == Some(0) {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: format!("{field_prefix}.artifact_cap_bytes"),
                detail: String::from("artifact_cap_bytes must stay positive when present"),
            });
        }
        if self.wallclock_cap_seconds == Some(0) {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: format!("{field_prefix}.wallclock_cap_seconds"),
                detail: String::from("wallclock_cap_seconds must stay positive when present"),
            });
        }
        if (self.artifact_cap_bytes.is_some() || self.wallclock_cap_seconds.is_some())
            && self.score_law_ref.is_none()
        {
            return Err(RemoteTrainingVisualizationError::MissingField {
                field: format!("{field_prefix}.score_law_ref"),
            });
        }
        Ok(())
    }
}

impl RemoteTrainingPrimaryScoreV2 {
    fn validate(&self, field_prefix: &str) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.score_metric_id.as_str(),
            format!("{field_prefix}.score_metric_id").as_str(),
        )?;
        ensure_nonempty(
            self.score_unit.as_str(),
            format!("{field_prefix}.score_unit").as_str(),
        )?;
        ensure_nonempty(
            self.score_summary.as_str(),
            format!("{field_prefix}.score_summary").as_str(),
        )?;
        validate_metric(
            self.score_value as f32,
            format!("{field_prefix}.score_value").as_str(),
        )?;
        Ok(())
    }
}

impl RemoteTrainingScoreDeltaV2 {
    fn validate(&self, field_prefix: &str) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.reference_id.as_str(),
            format!("{field_prefix}.reference_id").as_str(),
        )?;
        ensure_nonempty(
            self.score_metric_id.as_str(),
            format!("{field_prefix}.score_metric_id").as_str(),
        )?;
        ensure_nonempty(
            self.delta_summary.as_str(),
            format!("{field_prefix}.delta_summary").as_str(),
        )?;
        validate_metric(
            self.reference_score_value as f32,
            format!("{field_prefix}.reference_score_value").as_str(),
        )?;
        if !self.delta_value.is_finite() {
            return Err(RemoteTrainingVisualizationError::InvalidValue {
                field: format!("{field_prefix}.delta_value"),
                detail: String::from("delta_value must be finite"),
            });
        }
        Ok(())
    }
}

impl RemoteTrainingScoreSurfaceV2 {
    fn validate(&self, field_prefix: &str) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.semantic_summary.as_str(),
            format!("{field_prefix}.semantic_summary").as_str(),
        )?;
        let mut seen_reference_ids = BTreeSet::new();
        for (index, delta) in self.delta_rows.iter().enumerate() {
            delta.validate(format!("{field_prefix}.delta_rows[{index}]").as_str())?;
            if !seen_reference_ids.insert(delta.reference_id.as_str()) {
                return Err(RemoteTrainingVisualizationError::DuplicateValue {
                    field: format!("{field_prefix}.delta_rows.reference_id"),
                    value: delta.reference_id.clone(),
                });
            }
        }
        Ok(())
    }
}

impl RemoteTrainingVisualizationBundleV2 {
    pub fn from_v1_bundle(
        bundle: RemoteTrainingVisualizationBundle,
        track_semantics: RemoteTrainingTrackSemanticsV2,
        primary_score: Option<RemoteTrainingPrimaryScoreV2>,
    ) -> Result<Self, RemoteTrainingVisualizationError> {
        Self {
            schema_version: String::new(),
            bundle_id: bundle.bundle_id,
            provider: bundle.provider,
            profile_id: bundle.profile_id,
            lane_id: bundle.lane_id,
            run_id: bundle.run_id,
            repo_revision: bundle.repo_revision,
            track_semantics,
            primary_score,
            score_surface: None,
            result_classification: bundle.result_classification,
            refresh_contract: bundle.refresh_contract,
            series_status: bundle.series_status,
            series_unavailable_reason: bundle.series_unavailable_reason,
            timeline: bundle.timeline,
            summary: bundle.summary,
            heartbeat_series: bundle.heartbeat_series,
            loss_series: bundle.loss_series,
            math_series: bundle.math_series,
            runtime_series: bundle.runtime_series,
            gpu_series: bundle.gpu_series,
            distributed_series: bundle.distributed_series,
            event_series: bundle.event_series,
            source_artifacts: bundle.source_artifacts,
            bundle_digest: String::new(),
        }
        .finalize()
    }

    pub fn finalize(mut self) -> Result<Self, RemoteTrainingVisualizationError> {
        self.schema_version = String::from(REMOTE_TRAINING_VISUALIZATION_BUNDLE_V2_SCHEMA_VERSION);
        self.bundle_digest.clear();
        self.validate_payload()?;
        self.bundle_digest = stable_bundle_v2_digest(&self);
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        self.validate_payload()?;
        if self.bundle_digest != stable_bundle_v2_digest(self) {
            return Err(RemoteTrainingVisualizationError::DigestMismatch {
                kind: String::from("remote_training_visualization_bundle_v2"),
            });
        }
        Ok(())
    }

    fn validate_payload(&self) -> Result<(), RemoteTrainingVisualizationError> {
        check_string_match(
            self.schema_version.as_str(),
            REMOTE_TRAINING_VISUALIZATION_BUNDLE_V2_SCHEMA_VERSION,
            "remote_training_visualization_bundle_v2.schema_version",
        )?;
        ensure_nonempty(
            self.bundle_id.as_str(),
            "remote_training_visualization_bundle_v2.bundle_id",
        )?;
        ensure_nonempty(
            self.profile_id.as_str(),
            "remote_training_visualization_bundle_v2.profile_id",
        )?;
        ensure_nonempty(
            self.lane_id.as_str(),
            "remote_training_visualization_bundle_v2.lane_id",
        )?;
        ensure_nonempty(
            self.run_id.as_str(),
            "remote_training_visualization_bundle_v2.run_id",
        )?;
        ensure_nonempty(
            self.repo_revision.as_str(),
            "remote_training_visualization_bundle_v2.repo_revision",
        )?;
        self.track_semantics
            .validate("remote_training_visualization_bundle_v2.track_semantics")?;
        if let Some(score) = &self.primary_score {
            score.validate("remote_training_visualization_bundle_v2.primary_score")?;
            if self.track_semantics.score_law_ref.is_none() {
                return Err(RemoteTrainingVisualizationError::MissingField {
                    field: String::from(
                        "remote_training_visualization_bundle_v2.track_semantics.score_law_ref",
                    ),
                });
            }
        }
        if let Some(score_surface) = &self.score_surface {
            score_surface.validate("remote_training_visualization_bundle_v2.score_surface")?;
            if self.primary_score.is_none() {
                return Err(RemoteTrainingVisualizationError::MissingField {
                    field: String::from("remote_training_visualization_bundle_v2.primary_score"),
                });
            }
            if matches!(
                score_surface.score_closeout_posture,
                RemoteTrainingScoreCloseoutPostureV2::ScoreClosedOut
            ) && self.track_semantics.score_law_ref.is_none()
            {
                return Err(RemoteTrainingVisualizationError::MissingField {
                    field: String::from(
                        "remote_training_visualization_bundle_v2.track_semantics.score_law_ref",
                    ),
                });
            }
        }
        self.summary.validate()?;
        self.refresh_contract.validate(self.result_classification)?;
        validate_optional_nonempty(
            self.series_unavailable_reason.as_deref(),
            "remote_training_visualization_bundle_v2.series_unavailable_reason",
        )?;
        match self.series_status {
            RemoteTrainingSeriesStatus::Available => {
                if self.series_unavailable_reason.is_some() {
                    return Err(RemoteTrainingVisualizationError::InvalidValue {
                        field: String::from(
                            "remote_training_visualization_bundle_v2.series_unavailable_reason",
                        ),
                        detail: String::from(
                            "available series must not carry an unavailable reason",
                        ),
                    });
                }
                if self.loss_series.is_empty() {
                    return Err(RemoteTrainingVisualizationError::InvalidValue {
                        field: String::from("remote_training_visualization_bundle_v2.loss_series"),
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
                            "remote_training_visualization_bundle_v2.series_unavailable_reason",
                        ),
                    });
                }
            }
        }
        validate_monotonic_timeline_v2(self.timeline.as_slice())?;
        validate_monotonic_timestamps(
            self.heartbeat_series
                .iter()
                .map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle_v2.heartbeat_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.loss_series.iter().map(|sample| sample.elapsed_ms),
            "remote_training_visualization_bundle_v2.loss_series.elapsed_ms",
        )?;
        validate_monotonic_timestamps(
            self.math_series.iter().map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle_v2.math_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.runtime_series
                .iter()
                .map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle_v2.runtime_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.gpu_series.iter().map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle_v2.gpu_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.distributed_series
                .iter()
                .map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle_v2.distributed_series.observed_at_ms",
        )?;
        validate_monotonic_timestamps(
            self.event_series.iter().map(|sample| sample.observed_at_ms),
            "remote_training_visualization_bundle_v2.event_series.observed_at_ms",
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
                        "remote_training_visualization_bundle_v2.source_artifacts.artifact_role",
                    ),
                    value: artifact.artifact_role.clone(),
                });
            }
        }
        Ok(())
    }
}

impl RemoteTrainingRunIndexEntryV2 {
    fn from_v1_entry(
        entry: RemoteTrainingRunIndexEntry,
        track_semantics: RemoteTrainingTrackSemanticsV2,
        primary_score: Option<RemoteTrainingPrimaryScoreV2>,
        semantic_summary: String,
    ) -> Result<Self, RemoteTrainingVisualizationError> {
        let entry = Self {
            provider: entry.provider,
            profile_id: entry.profile_id,
            lane_id: entry.lane_id,
            run_id: entry.run_id,
            repo_revision: entry.repo_revision,
            track_semantics,
            primary_score,
            score_surface: None,
            result_classification: entry.result_classification,
            series_status: entry.series_status,
            series_unavailable_reason: entry.series_unavailable_reason,
            last_heartbeat_at_ms: entry.last_heartbeat_at_ms,
            bundle_artifact_uri: entry.bundle_artifact_uri,
            bundle_digest: entry.bundle_digest,
            semantic_summary,
        };
        entry.validate()?;
        Ok(entry)
    }

    pub(crate) fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        ensure_nonempty(
            self.profile_id.as_str(),
            "remote_training_run_index_v2.entries.profile_id",
        )?;
        ensure_nonempty(
            self.lane_id.as_str(),
            "remote_training_run_index_v2.entries.lane_id",
        )?;
        ensure_nonempty(
            self.run_id.as_str(),
            "remote_training_run_index_v2.entries.run_id",
        )?;
        ensure_nonempty(
            self.repo_revision.as_str(),
            "remote_training_run_index_v2.entries.repo_revision",
        )?;
        validate_optional_nonempty(
            self.series_unavailable_reason.as_deref(),
            "remote_training_run_index_v2.entries.series_unavailable_reason",
        )?;
        validate_optional_nonempty(
            self.bundle_artifact_uri.as_deref(),
            "remote_training_run_index_v2.entries.bundle_artifact_uri",
        )?;
        validate_optional_nonempty(
            self.bundle_digest.as_deref(),
            "remote_training_run_index_v2.entries.bundle_digest",
        )?;
        ensure_nonempty(
            self.semantic_summary.as_str(),
            "remote_training_run_index_v2.entries.semantic_summary",
        )?;
        self.track_semantics
            .validate("remote_training_run_index_v2.entries.track_semantics")?;
        if let Some(score) = &self.primary_score {
            score.validate("remote_training_run_index_v2.entries.primary_score")?;
            if self.track_semantics.score_law_ref.is_none() {
                return Err(RemoteTrainingVisualizationError::MissingField {
                    field: String::from(
                        "remote_training_run_index_v2.entries.track_semantics.score_law_ref",
                    ),
                });
            }
        }
        if let Some(score_surface) = &self.score_surface {
            score_surface.validate("remote_training_run_index_v2.entries.score_surface")?;
            if self.primary_score.is_none() {
                return Err(RemoteTrainingVisualizationError::MissingField {
                    field: String::from("remote_training_run_index_v2.entries.primary_score"),
                });
            }
        }
        match self.series_status {
            RemoteTrainingSeriesStatus::Available => {
                if self.series_unavailable_reason.is_some() {
                    return Err(RemoteTrainingVisualizationError::InvalidValue {
                        field: String::from(
                            "remote_training_run_index_v2.entries.series_unavailable_reason",
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
                            "remote_training_run_index_v2.entries.series_unavailable_reason",
                        ),
                    });
                }
            }
        }
        Ok(())
    }
}

impl RemoteTrainingRunIndexV2 {
    pub fn finalize(mut self) -> Result<Self, RemoteTrainingVisualizationError> {
        self.schema_version = String::from(REMOTE_TRAINING_RUN_INDEX_V2_SCHEMA_VERSION);
        self.index_digest.clear();
        self.validate_payload()?;
        self.index_digest = stable_run_index_v2_digest(&self);
        Ok(self)
    }

    pub fn validate(&self) -> Result<(), RemoteTrainingVisualizationError> {
        self.validate_payload()?;
        if self.index_digest != stable_run_index_v2_digest(self) {
            return Err(RemoteTrainingVisualizationError::DigestMismatch {
                kind: String::from("remote_training_run_index_v2"),
            });
        }
        Ok(())
    }

    fn validate_payload(&self) -> Result<(), RemoteTrainingVisualizationError> {
        check_string_match(
            self.schema_version.as_str(),
            REMOTE_TRAINING_RUN_INDEX_V2_SCHEMA_VERSION,
            "remote_training_run_index_v2.schema_version",
        )?;
        ensure_nonempty(
            self.index_id.as_str(),
            "remote_training_run_index_v2.index_id",
        )?;
        ensure_nonempty(self.detail.as_str(), "remote_training_run_index_v2.detail")?;
        if self.entries.is_empty() {
            return Err(RemoteTrainingVisualizationError::MissingField {
                field: String::from("remote_training_run_index_v2.entries"),
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
                    field: String::from("remote_training_run_index_v2.entries.run_identity"),
                    value: key,
                });
            }
        }
        Ok(())
    }
}

pub fn build_remote_training_visualization_bundle_v2(
    bundle: RemoteTrainingVisualizationBundleV2,
) -> Result<RemoteTrainingVisualizationBundleV2, RemoteTrainingVisualizationError> {
    bundle.finalize()
}

pub fn build_remote_training_run_index_v2(
    index: RemoteTrainingRunIndexV2,
) -> Result<RemoteTrainingRunIndexV2, RemoteTrainingVisualizationError> {
    index.finalize()
}

pub fn sample_parameter_golf_live_visualization_bundle_v2(
) -> Result<RemoteTrainingVisualizationBundleV2, RemoteTrainingVisualizationError> {
    let bundle = sample_parameter_golf_live_visualization_bundle()?;
    RemoteTrainingVisualizationBundleV2::from_v1_bundle(
        bundle,
        RemoteTrainingTrackSemanticsV2 {
            track_family: RemoteTrainingTrackFamilyV2::ParameterGolf,
            track_id: String::from(PARAMETER_GOLF_NON_RECORD_TRACK_ID),
            execution_class: RemoteTrainingExecutionClassV2::SingleNode,
            comparability_class: RemoteTrainingComparabilityClassV2::SameTrackComparable,
            proof_posture: RemoteTrainingProofPostureV2::RuntimeMeasured,
            public_equivalence_class: RemoteTrainingPublicEquivalenceClassV2::NotApplicable,
            score_law_ref: Some(String::from("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md")),
            artifact_cap_bytes: Some(PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES),
            wallclock_cap_seconds: None,
            semantic_summary: String::from(
                "Single-node PGOLF retains live optimizer telemetry under the non-record unlimited-compute 16MB submission track.",
            ),
        },
        None,
    )
}

pub fn sample_google_summary_only_visualization_bundle_v2(
) -> Result<RemoteTrainingVisualizationBundleV2, RemoteTrainingVisualizationError> {
    let bundle = sample_google_summary_only_visualization_bundle()?;
    RemoteTrainingVisualizationBundleV2::from_v1_bundle(
        bundle,
        RemoteTrainingTrackSemanticsV2 {
            track_family: RemoteTrainingTrackFamilyV2::Psion,
            track_id: String::from("psion.reference_pilot.single_node.v1"),
            execution_class: RemoteTrainingExecutionClassV2::SingleNode,
            comparability_class: RemoteTrainingComparabilityClassV2::NotComparable,
            proof_posture: RemoteTrainingProofPostureV2::SummaryOnly,
            public_equivalence_class: RemoteTrainingPublicEquivalenceClassV2::NotApplicable,
            score_law_ref: None,
            artifact_cap_bytes: None,
            wallclock_cap_seconds: None,
            semantic_summary: String::from(
                "Google single-node Psion keeps provider-neutral run identity and summary truth without inventing a score law or loss curve.",
            ),
        },
        None,
    )
}

pub fn sample_google_live_visualization_bundle_v2(
) -> Result<RemoteTrainingVisualizationBundleV2, RemoteTrainingVisualizationError> {
    let bundle = sample_google_live_visualization_bundle()?;
    RemoteTrainingVisualizationBundleV2::from_v1_bundle(
        bundle,
        RemoteTrainingTrackSemanticsV2 {
            track_family: RemoteTrainingTrackFamilyV2::Psion,
            track_id: String::from("psion.reference_pilot.accelerated_single_node.v1"),
            execution_class: RemoteTrainingExecutionClassV2::SingleNode,
            comparability_class: RemoteTrainingComparabilityClassV2::SameTrackComparable,
            proof_posture: RemoteTrainingProofPostureV2::RuntimeMeasured,
            public_equivalence_class: RemoteTrainingPublicEquivalenceClassV2::NotApplicable,
            score_law_ref: None,
            artifact_cap_bytes: None,
            wallclock_cap_seconds: None,
            semantic_summary: String::from(
                "Accelerated Google Psion retains measured live runtime telemetry every second without claiming PGOLF-style score comparability.",
            ),
        },
        None,
    )
}

pub fn sample_parameter_golf_distributed_live_visualization_bundle_v2(
) -> Result<RemoteTrainingVisualizationBundleV2, RemoteTrainingVisualizationError> {
    let bundle = sample_parameter_golf_distributed_live_visualization_bundle()?;
    RemoteTrainingVisualizationBundleV2::from_v1_bundle(
        bundle,
        RemoteTrainingTrackSemanticsV2 {
            track_family: RemoteTrainingTrackFamilyV2::ParameterGolf,
            track_id: String::from(PARAMETER_GOLF_NON_RECORD_TRACK_ID),
            execution_class: RemoteTrainingExecutionClassV2::DenseDistributed,
            comparability_class: RemoteTrainingComparabilityClassV2::SameTrackComparable,
            proof_posture: RemoteTrainingProofPostureV2::RuntimeMeasured,
            public_equivalence_class: RemoteTrainingPublicEquivalenceClassV2::NotApplicable,
            score_law_ref: Some(String::from("docs/PARAMETER_GOLF_NON_RECORD_SUBMISSION.md")),
            artifact_cap_bytes: Some(PARAMETER_GOLF_SUBMISSION_ARTIFACT_CAP_BYTES),
            wallclock_cap_seconds: None,
            semantic_summary: String::from(
                "Distributed PGOLF retains the same non-record track semantics while widening execution class to dense distributed runtime telemetry.",
            ),
        },
        None,
    )
}

pub fn sample_remote_training_run_index_v2(
) -> Result<RemoteTrainingRunIndexV2, RemoteTrainingVisualizationError> {
    let google_v1 = sample_google_summary_only_visualization_bundle()?;
    let google_v2 = sample_google_summary_only_visualization_bundle_v2()?;
    let google_live_v1 = sample_google_live_visualization_bundle()?;
    let google_live_v2 = sample_google_live_visualization_bundle_v2()?;
    let pgolf_v1 = sample_parameter_golf_live_visualization_bundle()?;
    let pgolf_v2 = sample_parameter_golf_live_visualization_bundle_v2()?;
    let distributed_v1 = sample_parameter_golf_distributed_live_visualization_bundle()?;
    let distributed_v2 = sample_parameter_golf_distributed_live_visualization_bundle_v2()?;
    let homegolf_v2 = build_parameter_golf_homegolf_visualization_bundle_v2()
        .map_err(map_homegolf_visualization_error)?;
    let xtrain_v2 = build_parameter_golf_xtrain_visualization_bundle_v2()
        .map_err(map_xtrain_visualization_error)?;

    build_remote_training_run_index_v2(RemoteTrainingRunIndexV2 {
        schema_version: String::new(),
        index_id: String::from("remote-training-run-index-sample-v2"),
        generated_at_ms: 1_742_846_405_000,
        entries: vec![
            RemoteTrainingRunIndexEntryV2::from_v1_entry(
                RemoteTrainingRunIndexEntry {
                    provider: google_v1.provider,
                    profile_id: google_v1.profile_id.clone(),
                    lane_id: google_v1.lane_id.clone(),
                    run_id: google_v1.run_id.clone(),
                    repo_revision: google_v1.repo_revision.clone(),
                    result_classification: google_v1.result_classification,
                    series_status: google_v1.series_status,
                    series_unavailable_reason: google_v1.series_unavailable_reason.clone(),
                    last_heartbeat_at_ms: google_v1.refresh_contract.last_heartbeat_at_ms,
                    bundle_artifact_uri: Some(String::from(
                        "fixtures/training_visualization/psion_google_summary_only_remote_training_visualization_bundle_v2.json",
                    )),
                    bundle_digest: Some(google_v2.bundle_digest.clone()),
                    summary_label: String::from("unused"),
                    detail: String::from("unused"),
                },
                google_v2.track_semantics.clone(),
                google_v2.primary_score.clone(),
                google_v2.track_semantics.semantic_summary.clone(),
            )?,
            RemoteTrainingRunIndexEntryV2::from_v1_entry(
                RemoteTrainingRunIndexEntry {
                    provider: google_live_v1.provider,
                    profile_id: google_live_v1.profile_id.clone(),
                    lane_id: google_live_v1.lane_id.clone(),
                    run_id: google_live_v1.run_id.clone(),
                    repo_revision: google_live_v1.repo_revision.clone(),
                    result_classification: google_live_v1.result_classification,
                    series_status: google_live_v1.series_status,
                    series_unavailable_reason: google_live_v1.series_unavailable_reason.clone(),
                    last_heartbeat_at_ms: google_live_v1.refresh_contract.last_heartbeat_at_ms,
                    bundle_artifact_uri: Some(String::from(
                        "fixtures/training_visualization/psion_google_live_remote_training_visualization_bundle_v2.json",
                    )),
                    bundle_digest: Some(google_live_v2.bundle_digest.clone()),
                    summary_label: String::from("unused"),
                    detail: String::from("unused"),
                },
                google_live_v2.track_semantics.clone(),
                google_live_v2.primary_score.clone(),
                google_live_v2.track_semantics.semantic_summary.clone(),
            )?,
            RemoteTrainingRunIndexEntryV2::from_v1_entry(
                RemoteTrainingRunIndexEntry {
                    provider: pgolf_v1.provider,
                    profile_id: pgolf_v1.profile_id.clone(),
                    lane_id: pgolf_v1.lane_id.clone(),
                    run_id: pgolf_v1.run_id.clone(),
                    repo_revision: pgolf_v1.repo_revision.clone(),
                    result_classification: pgolf_v1.result_classification,
                    series_status: pgolf_v1.series_status,
                    series_unavailable_reason: pgolf_v1.series_unavailable_reason.clone(),
                    last_heartbeat_at_ms: pgolf_v1.refresh_contract.last_heartbeat_at_ms,
                    bundle_artifact_uri: Some(String::from(
                        "fixtures/training_visualization/parameter_golf_live_remote_training_visualization_bundle_v2.json",
                    )),
                    bundle_digest: Some(pgolf_v2.bundle_digest.clone()),
                    summary_label: String::from("unused"),
                    detail: String::from("unused"),
                },
                pgolf_v2.track_semantics.clone(),
                pgolf_v2.primary_score.clone(),
                pgolf_v2.track_semantics.semantic_summary.clone(),
            )?,
            RemoteTrainingRunIndexEntryV2::from_v1_entry(
                RemoteTrainingRunIndexEntry {
                    provider: distributed_v1.provider,
                    profile_id: distributed_v1.profile_id.clone(),
                    lane_id: distributed_v1.lane_id.clone(),
                    run_id: distributed_v1.run_id.clone(),
                    repo_revision: distributed_v1.repo_revision.clone(),
                    result_classification: distributed_v1.result_classification,
                    series_status: distributed_v1.series_status,
                    series_unavailable_reason: distributed_v1.series_unavailable_reason.clone(),
                    last_heartbeat_at_ms: distributed_v1.refresh_contract.last_heartbeat_at_ms,
                    bundle_artifact_uri: Some(String::from(
                        "fixtures/training_visualization/parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v2.json",
                    )),
                    bundle_digest: Some(distributed_v2.bundle_digest.clone()),
                    summary_label: String::from("unused"),
                    detail: String::from("unused"),
                },
                distributed_v2.track_semantics.clone(),
                distributed_v2.primary_score.clone(),
                distributed_v2.track_semantics.semantic_summary.clone(),
            )?,
            build_parameter_golf_homegolf_run_index_entry_v2(&homegolf_v2)
                .map_err(map_homegolf_visualization_error)?,
            build_parameter_golf_xtrain_run_index_entry_v2(&xtrain_v2)
                .map_err(map_xtrain_visualization_error)?,
        ],
        detail: String::from(
            "The v2 run index keeps the shipped v1 discovery substrate readable while adding explicit track family, execution class, proof posture, comparability, score-law, HOMEGOLF score-surface semantics, and bounded XTRAIN train-to-infer posture.",
        ),
        index_digest: String::new(),
    })
}

fn stable_bundle_v2_digest(bundle: &RemoteTrainingVisualizationBundleV2) -> String {
    let mut canonical = bundle.clone();
    canonical.bundle_digest.clear();
    stable_digest_v2(b"remote_training_visualization_bundle_v2|", &canonical)
}

fn map_homegolf_visualization_error(
    error: crate::ParameterGolfHomegolfVisualizationError,
) -> RemoteTrainingVisualizationError {
    RemoteTrainingVisualizationError::InvalidValue {
        field: String::from("remote_training_run_index_v2.homegolf_entry"),
        detail: error.to_string(),
    }
}

fn map_xtrain_visualization_error(
    error: crate::ParameterGolfXtrainVisualizationError,
) -> RemoteTrainingVisualizationError {
    RemoteTrainingVisualizationError::InvalidValue {
        field: String::from("remote_training_run_index_v2.xtrain_entry"),
        detail: error.to_string(),
    }
}

fn stable_run_index_v2_digest(index: &RemoteTrainingRunIndexV2) -> String {
    let mut canonical = index.clone();
    canonical.index_digest.clear();
    stable_digest_v2(b"remote_training_run_index_v2|", &canonical)
}

fn stable_digest_v2<T>(prefix: &[u8], value: &T) -> String
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

fn validate_metric(value: f32, field: &str) -> Result<(), RemoteTrainingVisualizationError> {
    if !value.is_finite() || value < 0.0 {
        return Err(RemoteTrainingVisualizationError::InvalidValue {
            field: String::from(field),
            detail: String::from("metric must be finite and non-negative"),
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

fn validate_monotonic_timeline_v2(
    timeline: &[RemoteTrainingTimelineEntry],
) -> Result<(), RemoteTrainingVisualizationError> {
    if timeline.is_empty() {
        return Err(RemoteTrainingVisualizationError::MissingField {
            field: String::from("remote_training_visualization_bundle_v2.timeline"),
        });
    }
    validate_monotonic_timestamps(
        timeline.iter().map(|entry| entry.observed_at_ms),
        "remote_training_visualization_bundle_v2.timeline.observed_at_ms",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_parameter_golf_bundle_v2() -> RemoteTrainingVisualizationBundleV2 {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/parameter_golf_live_remote_training_visualization_bundle_v2.json"
        ))
        .expect("parameter golf v2 sample bundle should parse")
    }

    fn sample_google_bundle_v2() -> RemoteTrainingVisualizationBundleV2 {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/psion_google_summary_only_remote_training_visualization_bundle_v2.json"
        ))
        .expect("google summary v2 sample bundle should parse")
    }

    fn sample_index_v2() -> RemoteTrainingRunIndexV2 {
        serde_json::from_str(include_str!(
            "../../../fixtures/training_visualization/remote_training_run_index_v2.json"
        ))
        .expect("v2 run index should parse")
    }

    #[test]
    fn parameter_golf_v2_sample_bundle_stays_valid() -> Result<(), RemoteTrainingVisualizationError>
    {
        sample_parameter_golf_bundle_v2().validate()
    }

    #[test]
    fn google_v2_sample_bundle_stays_valid() -> Result<(), RemoteTrainingVisualizationError> {
        sample_google_bundle_v2().validate()
    }

    #[test]
    fn v2_run_index_stays_valid() -> Result<(), RemoteTrainingVisualizationError> {
        sample_index_v2().validate()
    }

    #[test]
    fn v2_track_semantics_require_score_law_when_caps_are_present() {
        let error = RemoteTrainingTrackSemanticsV2 {
            track_family: RemoteTrainingTrackFamilyV2::ParameterGolf,
            track_id: String::from(PARAMETER_GOLF_NON_RECORD_TRACK_ID),
            execution_class: RemoteTrainingExecutionClassV2::SingleNode,
            comparability_class: RemoteTrainingComparabilityClassV2::SameTrackComparable,
            proof_posture: RemoteTrainingProofPostureV2::RuntimeMeasured,
            public_equivalence_class: RemoteTrainingPublicEquivalenceClassV2::NotApplicable,
            score_law_ref: None,
            artifact_cap_bytes: Some(1),
            wallclock_cap_seconds: None,
            semantic_summary: String::from("sample"),
        }
        .validate("remote_training_visualization_bundle_v2.track_semantics")
        .expect_err("caps should require a score law reference");
        assert!(matches!(
            error,
            RemoteTrainingVisualizationError::MissingField { field }
                if field == "remote_training_visualization_bundle_v2.track_semantics.score_law_ref"
        ));
    }
}
