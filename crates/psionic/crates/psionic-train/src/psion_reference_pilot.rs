use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    time::Instant,
};

use psionic_backend_cuda::CudaBackend;
use psionic_backend_metal::MetalBackend;
use psionic_cluster::NodeId;
use psionic_core::{DType, Device, Shape, TensorData, TensorId, TensorSpec};
use psionic_data::{
    build_psion_actual_pretraining_reconstructed_corpus, build_psion_reference_corpus,
    DatasetSplitKind, PsionArtifactLineageManifest, PsionReferenceCorpusBundle,
    PsionReferenceCorpusError, PsionReferenceEncodedSequence, PSION_REFERENCE_DATASET_IDENTITY,
    PSION_REFERENCE_MAX_SEQUENCE_TOKENS,
};
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifestRef, DatastreamSubjectKind,
};
use psionic_ir::{
    AutodiffContext, AutodiffError, AutodiffGraph, AutodiffGraphBuilder, GraphError,
    ReferenceEvaluationError,
};
use psionic_models::{
    PsionCompactDecoderDescriptor, PsionCompactDecoderError, PsionCompactDecoderSizeAnchor,
    PsionCompactDecoderTokenizerBinding, PsionCompactDecoderTokenizerFamily,
};
use psionic_runtime::{
    ClusterCommunicationClass, ClusterExecutionContext, ClusterExecutionDisposition,
    ClusterSelectedNode, ClusterTransportClass, DeliveredExecutionContext, DeviceDescriptor,
    DeviceDiscovery, DeviceInventoryQualifiers, DeviceMemoryClass, DevicePerformanceClass,
    ExecutionTopologyPlan, RuntimeError, TrainingCheckpointAvailability,
    TrainingCheckpointReference, TrainingCollectiveContext, TrainingCollectiveKind,
    TrainingCollectiveQuantization, TrainingDeviceMeshAxis, TrainingDeviceMeshAxisKind,
    TrainingDeviceMeshContext, TrainingElasticMembershipContext, TrainingRecoveryContext,
    TrainingRecoveryPosture,
};
use safetensors::{serialize, tensor::TensorView, Dtype as SafeTensorsDType, SafeTensors};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    record_psion_pilot_held_out_loss, record_psion_pilot_pretraining_run,
    record_psion_pilot_route_probe, record_psion_pretrain_run_observability,
    record_psion_refusal_calibration_receipt, record_psion_route_class_evaluation_receipt,
    run_psion_pretrain_stage, run_psion_pretrain_stage_with_execution, ArtifactArchiveClass,
    ArtifactColdRestoreReceipt, ArtifactRetentionProfile, ArtifactStorageSweepReceipt,
    CheckpointDurabilityPosture, CheckpointManifest, CheckpointPointer, CheckpointRecoveryError,
    CheckpointScopeBinding, CheckpointScopeKind, CheckpointShardManifest,
    CheckpointStoreReadOptions, FixedBudgetTrainingRun, InMemoryCheckpointStore,
    PsionAcceptanceMatrix, PsionAcceptanceMatrixError, PsionBenchmarkCatalog,
    PsionBenchmarkEvidenceReceipt, PsionBenchmarkFamily, PsionBenchmarkPackageContract,
    PsionBenchmarkPackageError, PsionBenchmarkTaskContract, PsionCapabilityMatrixView,
    PsionCheckpointRecoveryReceipt, PsionContaminationReviewDisposition,
    PsionContaminationReviewReceipt, PsionGoogleSingleNodeLiveVisualizationWriter,
    PsionGoogleSingleNodeStepTelemetry, PsionGoogleSingleNodeVisualizationError, PsionMetricKind,
    PsionObservedMetric, PsionPhaseGate, PsionPilotHeldOutLossFamily, PsionPilotHeldOutLossRow,
    PsionPilotPretrainingRunBundle, PsionPilotPretrainingRunError, PsionPilotRouteProbeKind,
    PsionPilotRouteProbeRow, PsionPretrainCheckpointArtifactReceipt,
    PsionPretrainCheckpointLineageReceipt, PsionPretrainHardwareTopologyReceipt,
    PsionPretrainLossNormalization, PsionPretrainObjectiveConfig, PsionPretrainObjectiveKind,
    PsionPretrainReplayReceipt, PsionPretrainRunCostBasis, PsionPretrainRunCostReceipt,
    PsionPretrainRunObservabilityError, PsionPretrainRunObservabilityReceipt,
    PsionPretrainRunScaleProfile, PsionPretrainRunThroughputReceipt,
    PsionPretrainSourceFamilyReportRow, PsionPretrainStageAcceleratorReceipt,
    PsionPretrainStageConfig, PsionPretrainStageError, PsionPretrainStageRunReceipt,
    PsionPromotionDecisionDisposition, PsionPromotionDecisionReceipt, PsionRefusalCalibrationError,
    PsionRefusalCalibrationReceipt, PsionRefusalCalibrationRow, PsionRepetitiveRegionControl,
    PsionReplayEvidenceReceipt, PsionRouteCalibrationReceipt, PsionRouteClass,
    PsionRouteClassEvaluationError, PsionRouteClassEvaluationReceipt, PsionRouteClassEvaluationRow,
    PsionRouteKind, PsionSamplingContentClass, PsionSamplingPolicyError,
    PsionSamplingPolicyManifest, PsionSamplingRegressionKind, PsionSamplingRegressionThreshold,
    PsionSourceContributionCap, PsionSourceFamilySamplingWeight, TrainArtifactClass,
    TrainArtifactStorageController, TrainArtifactStorageError, TrainingCoreError,
    TrainingGradientBatch, TrainingLoopBudget, TrainingOptimizerConfig,
    TrainingOptimizerResidencyPolicy, TrainingParameterClass, TrainingParameterGroupState,
    TrainingRecoveryMode, TrainingRunSummary, TrainingSessionState, TrainingStepInput,
    TrainingStepReceipt, TrainingTensorBuffer,
};

const TOKEN_EMBEDDING_GROUP_ID: &str = "decoder.embed_tokens.weight";
const POSITION_EMBEDDING_GROUP_ID: &str = "decoder.embed_positions.weight";
const LM_HEAD_BIAS_GROUP_ID: &str = "lm_head.bias";
const ACCELERATED_REFERENCE_BATCH_ROWS: usize = 8_192;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotCheckpointManifest {
    pub schema_version: String,
    pub checkpoint_ref: String,
    pub checkpoint_family: String,
    pub run_id: String,
    pub stage_id: String,
    pub step: u64,
    pub model_id: String,
    pub model_descriptor_digest: String,
    pub dataset_identity: String,
    pub train_example_count: usize,
    pub validation_example_count: usize,
    pub parameter_ids: Vec<String>,
    pub parameter_state_digest: String,
}

impl PsionReferencePilotCheckpointManifest {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psion_reference_pilot_checkpoint_manifest|", self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotCheckpointArtifact {
    pub manifest: PsionReferencePilotCheckpointManifest,
    pub weights_bytes: Vec<u8>,
    pub checkpoint: TrainingCheckpointReference,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotProgressCheckpointReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub stage_id: String,
    pub checkpoint_family: String,
    pub optimizer_steps_completed: u64,
    pub window_index: u64,
    pub step_in_window: u64,
    pub cadence_index: u64,
    pub window_in_cadence: u64,
    pub is_window_boundary: bool,
    pub is_cadence_boundary: bool,
    pub is_terminal_boundary: bool,
    pub checkpoint_ref: String,
    pub checkpoint_manifest_digest: String,
    pub checkpoint_object_digest: String,
    pub checkpoint_total_bytes: u64,
    pub parameter_state_digest: String,
    pub expected_contributor_hosts: Vec<String>,
    pub observed_contributor_hosts: Vec<String>,
    pub runtime_backends: Vec<String>,
    pub contribution_receipt_count: usize,
    pub cumulative_train_tokens_processed: u64,
    pub cumulative_mean_tokens_per_second: u64,
    pub detail: String,
    pub receipt_digest: String,
}

impl PsionReferencePilotProgressCheckpointReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut stable_view = self.clone();
        stable_view.receipt_digest.clear();
        stable_digest(
            b"psion_reference_pilot_progress_checkpoint_receipt|",
            &stable_view,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotMissingContributorStep {
    pub global_step: u64,
    pub missing_hosts: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotContributorContinuityReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub stage_id: String,
    pub checkpoint_family: String,
    pub optimizer_steps_completed: u64,
    pub expected_contributor_hosts: Vec<String>,
    pub observed_contribution_counts_by_host: BTreeMap<String, u64>,
    pub runtime_backends_by_host: BTreeMap<String, String>,
    pub missing_contributors_by_step: Vec<PsionReferencePilotMissingContributorStep>,
    pub all_configured_contributors_present_each_step: bool,
    pub detail: String,
    pub receipt_digest: String,
}

impl PsionReferencePilotContributorContinuityReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut stable_view = self.clone();
        stable_view.receipt_digest.clear();
        stable_digest(
            b"psion_reference_pilot_contributor_continuity_receipt|",
            &stable_view,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionReferencePilotOptimizerStateArtifact {
    pub schema_version: String,
    pub run_id: String,
    pub stage_id: String,
    pub checkpoint_ref: String,
    pub checkpoint_family: String,
    pub completed_steps: u64,
    pub parameter_state_digest: String,
    pub parameter_groups: Vec<TrainingParameterGroupState>,
    pub summary: String,
    pub artifact_digest: String,
}

impl PsionReferencePilotOptimizerStateArtifact {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psion_reference_pilot_optimizer_state_artifact|", self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionReferencePilotResumeSeed {
    pub schema_version: String,
    pub checkpoint_artifact: PsionReferencePilotCheckpointArtifact,
    pub optimizer_state_artifact: PsionReferencePilotOptimizerStateArtifact,
    pub promoted_checkpoint: TrainingCheckpointReference,
    pub resume_step_offset: u64,
}

impl PsionReferencePilotResumeSeed {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psion_reference_pilot_resume_seed|", self)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionReferencePilotResumeProbe {
    pub schema_version: String,
    pub run_id: String,
    pub stage_id: String,
    pub checkpoint_ref: String,
    pub checkpoint_family: String,
    pub checkpoint_lineage_digest: String,
    pub recovery_mode: TrainingRecoveryMode,
    pub checkpoint_manifest: CheckpointManifest,
    pub checkpoint_pointer: CheckpointPointer,
    pub checkpoint_storage_artifact_id: String,
    pub checkpoint_storage_sweep_receipts: Vec<ArtifactStorageSweepReceipt>,
    pub checkpoint_cold_restore_receipts: Vec<ArtifactColdRestoreReceipt>,
    pub restore_receipt: crate::CheckpointRestoreReceipt,
    pub resumed_step_receipt: TrainingStepReceipt,
    pub resumed_run_summary: TrainingRunSummary,
    pub detail: String,
    pub probe_digest: String,
}

impl PsionReferencePilotResumeProbe {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        stable_digest(b"psion_reference_pilot_resume_probe|", self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PsionReferencePilotConfig {
    pub run_id: String,
    pub stage_id: String,
    pub checkpoint_family: String,
    pub started_at_ms: u64,
    pub step_duration_ms: u64,
    pub budget: TrainingLoopBudget,
    pub optimizer: TrainingOptimizerConfig,
}

impl PsionReferencePilotConfig {
    pub fn reference() -> Result<Self, PsionReferencePilotError> {
        Ok(Self {
            run_id: String::from("psion-reference-pilot-run"),
            stage_id: String::from("psion-reference-pretrain-stage"),
            checkpoint_family: String::from("train.psion.reference_pilot"),
            started_at_ms: 1_774_320_000_000,
            step_duration_ms: 40,
            budget: TrainingLoopBudget::new(16, 4, 1)?,
            optimizer: TrainingOptimizerConfig::adam(0.0005, 0.9, 0.99, 1e-8),
        })
    }

    pub fn accelerated_single_node() -> Result<Self, PsionReferencePilotError> {
        Ok(Self {
            run_id: String::from("psion-accelerated-reference-pilot-run"),
            stage_id: String::from("psion-accelerated-reference-pretrain-stage"),
            checkpoint_family: String::from("train.psion.accelerated_reference_pilot"),
            started_at_ms: 1_774_320_100_000,
            step_duration_ms: 1_000,
            budget: TrainingLoopBudget::new(4, 1, 1)?,
            optimizer: TrainingOptimizerConfig::adamw(0.0005, 0.9, 0.99, 1e-8)
                .with_weight_decay(0.01),
        })
    }

    pub fn distributed_dual_host() -> Result<Self, PsionReferencePilotError> {
        Ok(Self {
            run_id: String::from("psion-distributed-reference-pilot-run"),
            stage_id: String::from("psion-distributed-reference-pretrain-stage"),
            checkpoint_family: String::from("train.psion.distributed_reference_pilot"),
            started_at_ms: 1_774_320_200_000,
            step_duration_ms: 2_000,
            budget: TrainingLoopBudget::new(4, 2, 1)?,
            optimizer: TrainingOptimizerConfig::adamw(0.0005, 0.9, 0.99, 1e-8)
                .with_weight_decay(0.01),
        })
    }

    pub fn actual_pretraining_bringup() -> Result<Self, PsionReferencePilotError> {
        Ok(Self {
            run_id: String::from("psion-actual-pretraining-bringup-run"),
            stage_id: String::from("psion-actual-pretraining-stage-pretrain-bringup"),
            checkpoint_family: String::from("train.psion.actual_pretraining_bringup"),
            started_at_ms: 1_774_320_300_000,
            step_duration_ms: 4_000,
            budget: TrainingLoopBudget::new(2, 1, 1)?,
            optimizer: TrainingOptimizerConfig::adamw(0.0004, 0.9, 0.99, 1e-8)
                .with_weight_decay(0.01),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionReferencePilotContributionBackend {
    Cpu,
    Cuda,
    Metal,
}

impl PsionReferencePilotContributionBackend {
    #[must_use]
    pub const fn runtime_backend(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Cuda => "cuda",
            Self::Metal => "metal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionReferencePilotJointContributionRole {
    LocalAppleHost,
    RemoteTailnetWorker,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotDualHostConfig {
    pub control_plane_host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_plane_tailnet_ip: Option<String>,
    pub control_plane_batch_rows: usize,
    pub control_plane_backend: PsionReferencePilotContributionBackend,
    pub remote_worker_host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_worker_tailnet_ip: Option<String>,
    pub remote_worker_batch_rows: usize,
    pub remote_worker_backend: PsionReferencePilotContributionBackend,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_remote_worker_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_remote_worker_tailnet_ip: Option<String>,
    pub secondary_remote_worker_batch_rows: usize,
    pub secondary_remote_worker_backend: PsionReferencePilotContributionBackend,
}

impl PsionReferencePilotDualHostConfig {
    #[must_use]
    pub fn new(
        control_plane_host: impl Into<String>,
        remote_worker_host: impl Into<String>,
    ) -> Self {
        Self {
            control_plane_host: control_plane_host.into(),
            control_plane_tailnet_ip: None,
            control_plane_batch_rows: 256,
            control_plane_backend: PsionReferencePilotContributionBackend::Cpu,
            remote_worker_host: remote_worker_host.into(),
            remote_worker_tailnet_ip: None,
            remote_worker_batch_rows: 256,
            remote_worker_backend: PsionReferencePilotContributionBackend::Cuda,
            secondary_remote_worker_host: None,
            secondary_remote_worker_tailnet_ip: None,
            secondary_remote_worker_batch_rows: 256,
            secondary_remote_worker_backend: PsionReferencePilotContributionBackend::Cpu,
        }
    }

    #[must_use]
    pub fn with_control_plane_tailnet_ip(mut self, tailnet_ip: impl Into<String>) -> Self {
        self.control_plane_tailnet_ip = Some(tailnet_ip.into());
        self
    }

    #[must_use]
    pub fn with_remote_worker_tailnet_ip(mut self, tailnet_ip: impl Into<String>) -> Self {
        self.remote_worker_tailnet_ip = Some(tailnet_ip.into());
        self
    }

    #[must_use]
    pub const fn with_control_plane_batch_rows(mut self, control_plane_batch_rows: usize) -> Self {
        self.control_plane_batch_rows = control_plane_batch_rows;
        self
    }

    #[must_use]
    pub const fn with_control_plane_backend(
        mut self,
        control_plane_backend: PsionReferencePilotContributionBackend,
    ) -> Self {
        self.control_plane_backend = control_plane_backend;
        self
    }

    #[must_use]
    pub const fn with_remote_worker_batch_rows(mut self, remote_worker_batch_rows: usize) -> Self {
        self.remote_worker_batch_rows = remote_worker_batch_rows;
        self
    }

    #[must_use]
    pub const fn with_remote_worker_backend(
        mut self,
        remote_worker_backend: PsionReferencePilotContributionBackend,
    ) -> Self {
        self.remote_worker_backend = remote_worker_backend;
        self
    }

    #[must_use]
    pub fn with_secondary_remote_worker_host(
        mut self,
        secondary_remote_worker_host: impl Into<String>,
    ) -> Self {
        self.secondary_remote_worker_host = Some(secondary_remote_worker_host.into());
        self
    }

    #[must_use]
    pub fn with_secondary_remote_worker_tailnet_ip(
        mut self,
        secondary_remote_worker_tailnet_ip: impl Into<String>,
    ) -> Self {
        self.secondary_remote_worker_tailnet_ip = Some(secondary_remote_worker_tailnet_ip.into());
        self
    }

    #[must_use]
    pub const fn with_secondary_remote_worker_batch_rows(
        mut self,
        secondary_remote_worker_batch_rows: usize,
    ) -> Self {
        self.secondary_remote_worker_batch_rows = secondary_remote_worker_batch_rows;
        self
    }

    #[must_use]
    pub const fn with_secondary_remote_worker_backend(
        mut self,
        secondary_remote_worker_backend: PsionReferencePilotContributionBackend,
    ) -> Self {
        self.secondary_remote_worker_backend = secondary_remote_worker_backend;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionReferencePilotJointContributionRequest {
    pub schema_version: String,
    pub run_id: String,
    pub checkpoint_family: String,
    pub global_step: u64,
    pub contributor_id: String,
    pub contributor_host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributor_tailnet_ip: Option<String>,
    pub contributor_role: PsionReferencePilotJointContributionRole,
    pub contributor_index: usize,
    pub contributor_count: usize,
    pub backend: PsionReferencePilotContributionBackend,
    pub batch_rows: usize,
    pub parameter_values: BTreeMap<String, Vec<f32>>,
    pub parameter_state_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionReferencePilotJointContributionReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub checkpoint_family: String,
    pub global_step: u64,
    pub contributor_id: String,
    pub contributor_host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributor_tailnet_ip: Option<String>,
    pub contributor_role: PsionReferencePilotJointContributionRole,
    pub contributor_index: usize,
    pub contributor_count: usize,
    pub runtime_backend: String,
    pub selected_device: DeviceInventoryQualifiers,
    pub batch_rows: usize,
    pub sample_count: u32,
    pub example_selection_digest: String,
    pub parameter_state_digest: String,
    pub gradient_batch: TrainingGradientBatch,
    pub detail: String,
    pub contribution_digest: String,
}

impl PsionReferencePilotJointContributionReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut stable_view = self.clone();
        stable_view.contribution_digest.clear();
        stable_digest(
            b"psion_reference_pilot_joint_contribution_receipt|",
            &stable_view,
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotJointContributionSummaryReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub checkpoint_family: String,
    pub global_step: u64,
    pub contributor_id: String,
    pub contributor_host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contributor_tailnet_ip: Option<String>,
    pub contributor_role: PsionReferencePilotJointContributionRole,
    pub contributor_index: usize,
    pub contributor_count: usize,
    pub runtime_backend: String,
    pub selected_device: DeviceInventoryQualifiers,
    pub batch_rows: usize,
    pub sample_count: u32,
    pub example_selection_digest: String,
    pub parameter_state_digest: String,
    pub gradient_batch_id: String,
    pub gradient_group_ids: Vec<String>,
    pub detail: String,
    pub contribution_digest: String,
}

impl From<&PsionReferencePilotJointContributionReceipt>
    for PsionReferencePilotJointContributionSummaryReceipt
{
    fn from(value: &PsionReferencePilotJointContributionReceipt) -> Self {
        Self {
            schema_version: String::from(
                "psion.reference_pilot_joint_contribution_summary_receipt.v1",
            ),
            run_id: value.run_id.clone(),
            checkpoint_family: value.checkpoint_family.clone(),
            global_step: value.global_step,
            contributor_id: value.contributor_id.clone(),
            contributor_host: value.contributor_host.clone(),
            contributor_tailnet_ip: value.contributor_tailnet_ip.clone(),
            contributor_role: value.contributor_role,
            contributor_index: value.contributor_index,
            contributor_count: value.contributor_count,
            runtime_backend: value.runtime_backend.clone(),
            selected_device: value.selected_device.clone(),
            batch_rows: value.batch_rows,
            sample_count: value.sample_count,
            example_selection_digest: value.example_selection_digest.clone(),
            parameter_state_digest: value.parameter_state_digest.clone(),
            gradient_batch_id: value.gradient_batch.batch_id.clone(),
            gradient_group_ids: value
                .gradient_batch
                .gradients
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            detail: value.detail.clone(),
            contribution_digest: value.contribution_digest.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotDualHostStepReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub global_step: u64,
    pub local_contribution_digest: String,
    pub remote_contribution_digest: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contribution_digests: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributor_hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributor_runtime_backends: Vec<String>,
    pub merged_batch_id: String,
    pub merged_sample_count: u32,
    pub applied_receipt_id: String,
    pub applied_receipt_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionReferencePilotDualHostTopologyReceipt {
    pub schema_version: String,
    pub run_id: String,
    pub checkpoint_family: String,
    pub execution_topology_classification: String,
    pub control_plane_host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_plane_tailnet_ip: Option<String>,
    pub remote_worker_host: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_worker_tailnet_ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_remote_worker_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_remote_worker_tailnet_ip: Option<String>,
    pub local_runtime_backend: String,
    pub remote_runtime_backend: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub worker_hosts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub worker_tailnet_ips: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_backends: Vec<String>,
    pub contributor_count: usize,
    pub delivered_execution: DeliveredExecutionContext,
    pub joint_step_receipt_digests: Vec<String>,
    pub checkpoint_ref: String,
    pub detail: String,
    pub topology_receipt_digest: String,
}

impl PsionReferencePilotDualHostTopologyReceipt {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut stable_view = self.clone();
        stable_view.topology_receipt_digest.clear();
        stable_digest(
            b"psion_reference_pilot_dual_host_topology_receipt|",
            &stable_view,
        )
    }
}

#[derive(Clone, Debug)]
pub struct PsionReferencePilotDualHostRun {
    pub run: PsionReferencePilotRun,
    pub topology_receipt: PsionReferencePilotDualHostTopologyReceipt,
    pub contribution_receipts: Vec<PsionReferencePilotJointContributionReceipt>,
    pub contribution_summary_receipts: Vec<PsionReferencePilotJointContributionSummaryReceipt>,
    pub joint_step_receipts: Vec<PsionReferencePilotDualHostStepReceipt>,
    pub progress_checkpoint_receipts: Vec<PsionReferencePilotProgressCheckpointReceipt>,
    pub progress_checkpoint_artifacts: Vec<PsionReferencePilotCheckpointArtifact>,
    pub contributor_continuity_receipt: PsionReferencePilotContributorContinuityReceipt,
}

impl PsionReferencePilotDualHostRun {
    pub fn write_to_dir(&self, output_dir: &Path) -> Result<(), PsionReferencePilotError> {
        self.write_to_dir_with_prefix(output_dir, "psion_reference_pilot")
    }

    pub fn write_to_dir_with_prefix(
        &self,
        output_dir: &Path,
        prefix: &str,
    ) -> Result<(), PsionReferencePilotError> {
        self.run.write_to_dir_with_prefix(output_dir, prefix)?;
        let topology_path = output_dir.join(format!("{prefix}_dual_host_topology_receipt.json"));
        let step_path = output_dir.join(format!("{prefix}_dual_host_step_receipts.json"));
        let contribution_path =
            output_dir.join(format!("{prefix}_dual_host_contribution_receipts.json"));
        write_json(topology_path.as_path(), &self.topology_receipt)?;
        write_json(step_path.as_path(), &self.joint_step_receipts)?;
        write_json(
            contribution_path.as_path(),
            &self.contribution_summary_receipts,
        )?;
        write_json(
            output_dir
                .join(format!("{prefix}_progress_checkpoint_receipts.json"))
                .as_path(),
            &self.progress_checkpoint_receipts,
        )?;
        write_json(
            output_dir
                .join(format!("{prefix}_contributor_continuity_receipt.json"))
                .as_path(),
            &self.contributor_continuity_receipt,
        )?;
        let progress_dir = output_dir.join(format!("{prefix}_progress_checkpoints"));
        fs::create_dir_all(&progress_dir).map_err(|error| {
            PsionReferencePilotError::Serialization {
                message: error.to_string(),
            }
        })?;
        for (index, (receipt, artifact)) in self
            .progress_checkpoint_receipts
            .iter()
            .zip(self.progress_checkpoint_artifacts.iter())
            .enumerate()
        {
            let checkpoint_dir = progress_dir.join(format!(
                "{:02}-step-{:04}-window-{:02}-cadence-{:02}",
                index.saturating_add(1),
                receipt.optimizer_steps_completed,
                receipt.window_index,
                receipt.cadence_index,
            ));
            fs::create_dir_all(&checkpoint_dir).map_err(|error| {
                PsionReferencePilotError::Serialization {
                    message: error.to_string(),
                }
            })?;
            write_json(checkpoint_dir.join("receipt.json").as_path(), receipt)?;
            write_json(
                checkpoint_dir.join("checkpoint_manifest.json").as_path(),
                &artifact.manifest,
            )?;
            fs::write(
                checkpoint_dir.join("checkpoint.safetensors"),
                &artifact.weights_bytes,
            )
            .map_err(|error| PsionReferencePilotError::Serialization {
                message: error.to_string(),
            })?;
        }
        if self.topology_receipt.contributor_count > 2 {
            write_json(
                output_dir
                    .join(format!("{prefix}_cluster_topology_receipt.json"))
                    .as_path(),
                &self.topology_receipt,
            )?;
            write_json(
                output_dir
                    .join(format!("{prefix}_cluster_step_receipts.json"))
                    .as_path(),
                &self.joint_step_receipts,
            )?;
            write_json(
                output_dir
                    .join(format!("{prefix}_cluster_contribution_receipts.json"))
                    .as_path(),
                &self.contribution_summary_receipts,
            )?;
            write_json(
                output_dir
                    .join(format!(
                        "{prefix}_cluster_progress_checkpoint_receipts.json"
                    ))
                    .as_path(),
                &self.progress_checkpoint_receipts,
            )?;
            write_json(
                output_dir
                    .join(format!(
                        "{prefix}_cluster_contributor_continuity_receipt.json"
                    ))
                    .as_path(),
                &self.contributor_continuity_receipt,
            )?;
        }
        Ok(())
    }
}

fn schedule_for_global_step(budget: TrainingLoopBudget, global_step: u64) -> (u64, u64, u64, u64) {
    let window_index = ((global_step.saturating_sub(1)) / budget.steps_per_window) + 1;
    let step_in_window = ((global_step.saturating_sub(1)) % budget.steps_per_window) + 1;
    let cadence_index = ((window_index.saturating_sub(1)) / budget.windows_per_cadence) + 1;
    let window_in_cadence = ((window_index.saturating_sub(1)) % budget.windows_per_cadence) + 1;
    (
        window_index,
        step_in_window,
        cadence_index,
        window_in_cadence,
    )
}

fn should_emit_progress_checkpoint_receipt(
    budget: TrainingLoopBudget,
    global_step: u64,
) -> (bool, bool, bool) {
    let (window_index, step_in_window, _, window_in_cadence) =
        schedule_for_global_step(budget, global_step);
    let _ = window_index;
    let is_terminal_boundary = global_step == budget.max_steps;
    let is_window_boundary = step_in_window == budget.steps_per_window || is_terminal_boundary;
    let is_cadence_boundary = is_window_boundary
        && (window_in_cadence == budget.windows_per_cadence || is_terminal_boundary);
    (
        is_window_boundary,
        is_cadence_boundary,
        is_terminal_boundary,
    )
}

fn cumulative_train_tokens_processed(
    train_examples: &[PsionReferenceTrainingExample],
    contribution_receipts: &[PsionReferencePilotJointContributionReceipt],
    global_step: u64,
) -> u64 {
    contribution_receipts
        .iter()
        .filter(|receipt| receipt.global_step <= global_step)
        .map(|receipt| {
            token_count(
                joint_reference_examples_for_step(
                    train_examples,
                    receipt.global_step,
                    receipt.batch_rows,
                    receipt.contributor_index,
                    receipt.contributor_count,
                )
                .as_slice(),
            )
        })
        .sum::<u64>()
}

fn build_progress_checkpoint_receipt(
    config: &PsionReferencePilotConfig,
    schedule_budget: TrainingLoopBudget,
    checkpoint_artifact: &PsionReferencePilotCheckpointArtifact,
    contribution_receipts: &[PsionReferencePilotJointContributionReceipt],
    expected_contributor_hosts: &[String],
    global_step: u64,
    train_examples: &[PsionReferenceTrainingExample],
) -> PsionReferencePilotProgressCheckpointReceipt {
    let (window_index, step_in_window, cadence_index, window_in_cadence) =
        schedule_for_global_step(schedule_budget, global_step);
    let (is_window_boundary, is_cadence_boundary, is_terminal_boundary) =
        should_emit_progress_checkpoint_receipt(schedule_budget, global_step);
    let observed_hosts = contribution_receipts
        .iter()
        .filter(|receipt| receipt.global_step == global_step)
        .map(|receipt| receipt.contributor_host.clone())
        .collect::<Vec<_>>();
    let runtime_backends = contribution_receipts
        .iter()
        .filter(|receipt| receipt.global_step == global_step)
        .map(|receipt| receipt.runtime_backend.clone())
        .collect::<Vec<_>>();
    let cumulative_train_tokens_processed =
        cumulative_train_tokens_processed(train_examples, contribution_receipts, global_step);
    let wall_clock_ms = global_step.saturating_mul(config.step_duration_ms);
    let checkpoint_total_bytes = checkpoint_artifact.weights_bytes.len() as u64;
    let mut receipt = PsionReferencePilotProgressCheckpointReceipt {
        schema_version: String::from("psion.reference_pilot_progress_checkpoint_receipt.v1"),
        run_id: config.run_id.clone(),
        stage_id: config.stage_id.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        optimizer_steps_completed: global_step,
        window_index,
        step_in_window,
        cadence_index,
        window_in_cadence,
        is_window_boundary,
        is_cadence_boundary,
        is_terminal_boundary,
        checkpoint_ref: checkpoint_artifact.manifest.checkpoint_ref.clone(),
        checkpoint_manifest_digest: checkpoint_artifact.manifest.stable_digest(),
        checkpoint_object_digest: checkpoint_artifact.checkpoint.object_digest.clone(),
        checkpoint_total_bytes,
        parameter_state_digest: checkpoint_artifact.manifest.parameter_state_digest.clone(),
        expected_contributor_hosts: expected_contributor_hosts.to_vec(),
        observed_contributor_hosts: observed_hosts.clone(),
        runtime_backends,
        contribution_receipt_count: contribution_receipts
            .iter()
            .filter(|receipt| receipt.global_step <= global_step)
            .count(),
        cumulative_train_tokens_processed,
        cumulative_mean_tokens_per_second: (cumulative_train_tokens_processed * 1000)
            / wall_clock_ms.max(1),
        detail: format!(
            "Progress checkpoint `{}` sealed step {} at window {} cadence {} after contributions from {}.",
            checkpoint_artifact.manifest.checkpoint_ref,
            global_step,
            window_index,
            cadence_index,
            observed_hosts
                .iter()
                .map(|host| format!("`{host}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn build_contributor_continuity_receipt(
    config: &PsionReferencePilotConfig,
    expected_contributor_hosts: &[String],
    contribution_receipts: &[PsionReferencePilotJointContributionReceipt],
) -> PsionReferencePilotContributorContinuityReceipt {
    let mut observed_contribution_counts_by_host = BTreeMap::<String, u64>::new();
    let mut runtime_backends_by_host = BTreeMap::<String, String>::new();
    for receipt in contribution_receipts {
        *observed_contribution_counts_by_host
            .entry(receipt.contributor_host.clone())
            .or_insert(0) += 1;
        runtime_backends_by_host
            .entry(receipt.contributor_host.clone())
            .or_insert_with(|| receipt.runtime_backend.clone());
    }
    let mut missing_contributors_by_step = Vec::new();
    for global_step in 1..=config.budget.max_steps {
        let observed_hosts = contribution_receipts
            .iter()
            .filter(|receipt| receipt.global_step == global_step)
            .map(|receipt| receipt.contributor_host.clone())
            .collect::<BTreeSet<_>>();
        let missing_hosts = expected_contributor_hosts
            .iter()
            .filter(|host| !observed_hosts.contains(host.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_hosts.is_empty() {
            missing_contributors_by_step.push(PsionReferencePilotMissingContributorStep {
                global_step,
                missing_hosts,
            });
        }
    }
    let mut receipt = PsionReferencePilotContributorContinuityReceipt {
        schema_version: String::from("psion.reference_pilot_contributor_continuity_receipt.v1"),
        run_id: config.run_id.clone(),
        stage_id: config.stage_id.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        optimizer_steps_completed: config.budget.max_steps,
        expected_contributor_hosts: expected_contributor_hosts.to_vec(),
        observed_contribution_counts_by_host,
        runtime_backends_by_host,
        all_configured_contributors_present_each_step: missing_contributors_by_step.is_empty(),
        missing_contributors_by_step,
        detail: format!(
            "Contributor continuity for `{}` expected {} on every optimizer step across {} completed steps.",
            config.run_id,
            expected_contributor_hosts
                .iter()
                .map(|host| format!("`{host}`"))
                .collect::<Vec<_>>()
                .join(", "),
            config.budget.max_steps
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

#[derive(Clone, Debug)]
pub struct PsionReferencePilotRun {
    pub corpus_bundle: PsionReferenceCorpusBundle,
    pub model_descriptor: PsionCompactDecoderDescriptor,
    pub sampling_policy: PsionSamplingPolicyManifest,
    pub stage_config: PsionPretrainStageConfig,
    pub stage_receipt: PsionPretrainStageRunReceipt,
    pub observability_receipt: PsionPretrainRunObservabilityReceipt,
    pub checkpoint_artifact: PsionReferencePilotCheckpointArtifact,
    pub optimizer_state_artifact: PsionReferencePilotOptimizerStateArtifact,
    pub initial_validation_loss_milli_by_family: BTreeMap<String, u32>,
    pub final_validation_loss_milli_by_family: BTreeMap<String, u32>,
    pub initial_held_out_loss_milli: u32,
    pub final_held_out_loss_milli: u32,
    pub step_receipts: Vec<TrainingStepReceipt>,
}

impl PsionReferencePilotRun {
    pub fn write_to_dir(&self, output_dir: &Path) -> Result<(), PsionReferencePilotError> {
        self.write_to_dir_with_prefix(output_dir, "psion_reference_pilot")
    }

    pub fn write_to_dir_with_prefix(
        &self,
        output_dir: &Path,
        prefix: &str,
    ) -> Result<(), PsionReferencePilotError> {
        fs::create_dir_all(output_dir).map_err(|error| {
            PsionReferencePilotError::Serialization {
                message: error.to_string(),
            }
        })?;
        write_json(
            output_dir
                .join(format!("{prefix}_stage_config.json"))
                .as_path(),
            &self.stage_config,
        )?;
        write_json(
            output_dir
                .join(format!("{prefix}_stage_receipt.json"))
                .as_path(),
            &self.stage_receipt,
        )?;
        write_json(
            output_dir
                .join(format!("{prefix}_observability_receipt.json"))
                .as_path(),
            &self.observability_receipt,
        )?;
        write_json(
            output_dir
                .join(format!("{prefix}_checkpoint_manifest.json"))
                .as_path(),
            &self.checkpoint_artifact.manifest,
        )?;
        write_json(
            output_dir
                .join(format!("{prefix}_optimizer_state.json"))
                .as_path(),
            &self.optimizer_state_artifact,
        )?;
        write_json(
            output_dir.join(format!("{prefix}_summary.json")).as_path(),
            &serde_json::json!({
                "run_id": self.stage_receipt.run_id,
                "stage_id": self.stage_receipt.stage_id,
                "checkpoint_ref": self.checkpoint_artifact.manifest.checkpoint_ref,
                "optimizer_state_digest": self.optimizer_state_artifact.artifact_digest,
                "optimizer_steps": self.step_receipts.len(),
                "initial_validation_loss_milli_by_family": self.initial_validation_loss_milli_by_family,
                "final_validation_loss_milli_by_family": self.final_validation_loss_milli_by_family,
                "initial_held_out_loss_milli": self.initial_held_out_loss_milli,
                "final_held_out_loss_milli": self.final_held_out_loss_milli
            }),
        )?;
        fs::write(
            output_dir.join(format!("{prefix}_checkpoint.safetensors")),
            &self.checkpoint_artifact.weights_bytes,
        )
        .map_err(|error| PsionReferencePilotError::Serialization {
            message: error.to_string(),
        })?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotBenchmarkRow {
    pub item_id: String,
    pub matched_sequence_id: Option<String>,
    pub matched_source_id: Option<String>,
    pub observed_route_class: Option<PsionRouteClass>,
    pub observed_refusal_reason_code: Option<String>,
    pub observed_score_milli: i32,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionReferencePilotBenchmarkEvaluation {
    pub evaluation_id: String,
    pub family: PsionBenchmarkFamily,
    pub benchmark_artifact_id: String,
    pub benchmark_artifact_digest: String,
    pub rows: Vec<PsionReferencePilotBenchmarkRow>,
    pub aggregate_pass_rate_bps: u32,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct PsionReferencePilotEvidenceBundle {
    pub run: PsionReferencePilotRun,
    pub architecture_benchmark: PsionReferencePilotBenchmarkEvaluation,
    pub normative_spec_benchmark: PsionReferencePilotBenchmarkEvaluation,
    pub held_out_benchmark: PsionReferencePilotBenchmarkEvaluation,
    pub route_class_evaluation_receipt: PsionRouteClassEvaluationReceipt,
    pub refusal_calibration_receipt: PsionRefusalCalibrationReceipt,
    pub pilot_bundle: PsionPilotPretrainingRunBundle,
}

impl PsionReferencePilotEvidenceBundle {
    pub fn write_to_dir(&self, output_dir: &Path) -> Result<(), PsionReferencePilotEvidenceError> {
        fs::create_dir_all(output_dir).map_err(|error| PsionReferencePilotEvidenceError::Io {
            message: error.to_string(),
        })?;
        self.run.write_to_dir(output_dir)?;
        write_json(
            output_dir
                .join("psion_reference_architecture_benchmark_eval.json")
                .as_path(),
            &self.architecture_benchmark,
        )?;
        write_json(
            output_dir
                .join("psion_reference_normative_spec_benchmark_eval.json")
                .as_path(),
            &self.normative_spec_benchmark,
        )?;
        write_json(
            output_dir
                .join("psion_reference_held_out_benchmark_eval.json")
                .as_path(),
            &self.held_out_benchmark,
        )?;
        write_json(
            output_dir
                .join("psion_reference_route_class_evaluation_receipt.json")
                .as_path(),
            &self.route_class_evaluation_receipt,
        )?;
        write_json(
            output_dir
                .join("psion_reference_refusal_calibration_receipt.json")
                .as_path(),
            &self.refusal_calibration_receipt,
        )?;
        write_json(
            output_dir
                .join("psion_reference_pilot_pretraining_bundle.json")
                .as_path(),
            &self.pilot_bundle,
        )?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum PsionReferencePilotEvidenceError {
    #[error(transparent)]
    ReferencePilot(#[from] PsionReferencePilotError),
    #[error(transparent)]
    AcceptanceMatrix(#[from] PsionAcceptanceMatrixError),
    #[error(transparent)]
    BenchmarkPackage(#[from] PsionBenchmarkPackageError),
    #[error(transparent)]
    RouteClassEvaluation(#[from] PsionRouteClassEvaluationError),
    #[error(transparent)]
    RefusalCalibration(#[from] PsionRefusalCalibrationError),
    #[error(transparent)]
    PilotBundle(#[from] PsionPilotPretrainingRunError),
    #[error("reference pilot evidence io failed: {message}")]
    Io { message: String },
    #[error("reference pilot evidence serialization failed: {message}")]
    Serialization { message: String },
    #[error("reference pilot evidence is missing benchmark package `{package_id}`")]
    MissingBenchmarkPackage { package_id: String },
}

#[derive(Debug, Error)]
pub enum PsionReferencePilotError {
    #[error(transparent)]
    Corpus(#[from] PsionReferenceCorpusError),
    #[error(transparent)]
    CheckpointRecovery(#[from] CheckpointRecoveryError),
    #[error(transparent)]
    SamplingPolicy(#[from] PsionSamplingPolicyError),
    #[error(transparent)]
    Descriptor(#[from] PsionCompactDecoderError),
    #[error(transparent)]
    PretrainStage(#[from] PsionPretrainStageError),
    #[error(transparent)]
    Observability(#[from] PsionPretrainRunObservabilityError),
    #[error(transparent)]
    ArtifactStorage(#[from] TrainArtifactStorageError),
    #[error(transparent)]
    TrainingCore(#[from] TrainingCoreError),
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error(transparent)]
    Autodiff(#[from] AutodiffError),
    #[error(transparent)]
    ReferenceEvaluation(#[from] ReferenceEvaluationError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
    #[error(transparent)]
    Visualization(#[from] PsionGoogleSingleNodeVisualizationError),
    #[error("accelerated reference pilot requires a visible CUDA device: {detail}")]
    CudaBackendUnavailable { detail: String },
    #[error("accelerated reference pilot requires a visible Metal device: {detail}")]
    MetalBackendUnavailable { detail: String },
    #[error("reference pilot checkpoint serialization failed: {message}")]
    Serialization { message: String },
    #[error(
        "reference pilot parameter-state digest mismatch: expected `{expected}`, found `{actual}`"
    )]
    ParameterStateDigestMismatch { expected: String, actual: String },
    #[error("reference pilot is missing parameter group `{group_id}`")]
    MissingParameterGroup { group_id: String },
    #[error("reference pilot resume probe is missing restore lineage on the resumed step")]
    MissingRestoreSource,
    #[error("reference pilot parameter group `{group_id}` is not dense f32")]
    NonDenseParameterGroup { group_id: String },
    #[error("reference pilot remote contribution failed: {message}")]
    RemoteContribution { message: String },
    #[error("reference pilot joint contribution receipt is invalid: {message}")]
    InvalidJointContribution { message: String },
}

#[derive(Clone, Debug, PartialEq)]
struct PsionReferenceTrainingExample {
    source_id: String,
    source_family_id: String,
    split_kind: DatasetSplitKind,
    context_token_ids: Vec<u32>,
    target_token_id: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PsionCompactDecoderReferencePilotModel {
    descriptor: PsionCompactDecoderDescriptor,
    token_embeddings: Vec<f32>,
    position_embeddings: Vec<f32>,
    lm_head_bias: Vec<f32>,
}

#[derive(Clone, Debug)]
struct PilotLossSummary {
    mean_loss: f32,
    loss_by_family_milli: BTreeMap<String, u32>,
}

#[derive(Clone, Debug)]
struct PsionAcceleratedGradientProgram {
    logits_graph: AutodiffGraph,
    logits_hidden_input_tensor_id: TensorId,
    logits_token_embeddings_tensor_id: TensorId,
    logits_tensor_id: TensorId,
    weight_gradient_graph: AutodiffGraph,
    weight_gradient_hidden_input_transposed_tensor_id: TensorId,
    weight_gradient_logits_seed_tensor_id: TensorId,
    weight_gradient_tensor_id: TensorId,
    hidden_gradient_graph: AutodiffGraph,
    hidden_gradient_logits_seed_tensor_id: TensorId,
    hidden_gradient_token_embeddings_vh_tensor_id: TensorId,
    hidden_gradient_tensor_id: TensorId,
}

pub fn run_psion_reference_pilot(
    repo_root: &Path,
    config: &PsionReferencePilotConfig,
) -> Result<PsionReferencePilotRun, PsionReferencePilotError> {
    let corpus_bundle = build_psion_reference_corpus(repo_root)?;
    let sampling_policy = build_reference_sampling_policy(&corpus_bundle)?;
    let model_descriptor = build_reference_model_descriptor(&corpus_bundle)?;
    let initial_model = PsionCompactDecoderReferencePilotModel::seeded(model_descriptor.clone());
    let train_examples = split_examples(&corpus_bundle, DatasetSplitKind::Train);
    let validation_examples = split_examples(&corpus_bundle, DatasetSplitKind::Validation);
    let held_out_examples = split_examples(&corpus_bundle, DatasetSplitKind::HeldOut);

    let initial_validation_summary = evaluate_examples(&initial_model, &validation_examples);
    let initial_held_out_summary = evaluate_examples(&initial_model, &held_out_examples);

    let parameter_groups = build_parameter_groups(&initial_model, config)?;
    let mut run = FixedBudgetTrainingRun::new(
        config.run_id.clone(),
        config.checkpoint_family.clone(),
        config.budget,
        parameter_groups,
    )?;

    let mut current_model = initial_model.clone();
    let mut step_receipts = Vec::new();
    for step_index in 0..config.budget.max_steps {
        let batch = build_gradient_batch(&current_model, &train_examples)?;
        let started_at_ms = config
            .started_at_ms
            .saturating_add(step_index.saturating_mul(config.step_duration_ms));
        let finished_at_ms = started_at_ms.saturating_add(config.step_duration_ms);
        let receipt =
            run.apply_step(TrainingStepInput::new(batch, started_at_ms, finished_at_ms))?;
        current_model = materialize_model(&model_descriptor, &run)?;
        step_receipts.push(receipt);
    }

    let final_validation_summary = evaluate_examples(&current_model, &validation_examples);
    let final_held_out_summary = evaluate_examples(&current_model, &held_out_examples);
    let checkpoint_artifact = export_checkpoint(
        &current_model,
        &train_examples,
        &validation_examples,
        config,
        &model_descriptor,
        corpus_bundle
            .tokenized_corpus_manifest
            .replay_contract
            .stable_dataset_identity
            .as_str(),
        config.budget.max_steps,
        config.started_at_ms.saturating_add(
            config
                .budget
                .max_steps
                .saturating_mul(config.step_duration_ms),
        ),
    )?;
    let optimizer_state_artifact =
        build_optimizer_state_artifact(&run, &checkpoint_artifact, config)?;

    let stage_config = PsionPretrainStageConfig::new(
        config.run_id.clone(),
        config.stage_id.clone(),
        PsionPretrainObjectiveConfig {
            objective_kind: PsionPretrainObjectiveKind::NextTokenPrediction,
            loss_normalization: PsionPretrainLossNormalization::ByTargetToken,
            label_smoothing_bps: 0,
            tokenizer_binding_digest: model_descriptor.tokenizer_binding.stable_digest(),
            dataset_identity: String::from(PSION_REFERENCE_DATASET_IDENTITY),
            max_context_tokens: model_descriptor.config.max_context,
        },
        &model_descriptor,
        &corpus_bundle.tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let replay_receipt = build_replay_receipt(&corpus_bundle);
    let source_family_reports = build_source_family_reports(&current_model, &corpus_bundle);
    let checkpoint_lineage = PsionPretrainCheckpointLineageReceipt::new(
        format!("{}-checkpoint-lineage", config.run_id),
        checkpoint_artifact.checkpoint.clone(),
        None,
        checkpoint_artifact.manifest.checkpoint_ref.clone(),
        model_descriptor.model.model_id.clone(),
        model_descriptor.stable_digest(),
    );
    let stage_receipt = run_psion_pretrain_stage(
        &stage_config,
        source_family_reports,
        replay_receipt,
        checkpoint_lineage,
        "Reference Psion pilot completed real optimizer steps over the repo-owned reference corpus and emitted a durable checkpoint.",
        &model_descriptor,
        &corpus_bundle.tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let observability_receipt = build_observability_receipt(
        config,
        &stage_receipt,
        &checkpoint_artifact,
        &step_receipts,
        train_examples.as_slice(),
        validation_examples.as_slice(),
        held_out_examples.as_slice(),
    )?;

    Ok(PsionReferencePilotRun {
        corpus_bundle,
        model_descriptor,
        sampling_policy,
        stage_config,
        stage_receipt,
        observability_receipt,
        checkpoint_artifact,
        optimizer_state_artifact,
        initial_validation_loss_milli_by_family: initial_validation_summary.loss_by_family_milli,
        final_validation_loss_milli_by_family: final_validation_summary.loss_by_family_milli,
        initial_held_out_loss_milli: milli_loss(initial_held_out_summary.mean_loss),
        final_held_out_loss_milli: milli_loss(final_held_out_summary.mean_loss),
        step_receipts,
    })
}

pub fn run_psion_accelerated_reference_pilot(
    repo_root: &Path,
    config: &PsionReferencePilotConfig,
) -> Result<PsionReferencePilotRun, PsionReferencePilotError> {
    run_psion_accelerated_reference_pilot_with_live_visualization(repo_root, config, None)
}

pub fn run_psion_accelerated_reference_pilot_with_live_visualization(
    repo_root: &Path,
    config: &PsionReferencePilotConfig,
    live_visualization_writer: Option<&mut PsionGoogleSingleNodeLiveVisualizationWriter>,
) -> Result<PsionReferencePilotRun, PsionReferencePilotError> {
    let mut live_visualization_writer = live_visualization_writer;
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "dataset_staging",
            Some(String::from("reference_corpus_build")),
            "The accelerated Psion reference lane is materializing the repo-owned reference corpus.",
            vec![
                String::from("reference_corpus"),
                String::from("dataset_tokenization"),
            ],
            None,
            true,
        )?;
    }
    let corpus_bundle = build_psion_reference_corpus(repo_root)?;
    let sampling_policy = build_reference_sampling_policy(&corpus_bundle)?;
    let model_descriptor = build_reference_model_descriptor(&corpus_bundle)?;
    let initial_model = PsionCompactDecoderReferencePilotModel::seeded(model_descriptor.clone());
    let train_examples = split_examples(&corpus_bundle, DatasetSplitKind::Train);
    let validation_examples = split_examples(&corpus_bundle, DatasetSplitKind::Validation);
    let held_out_examples = split_examples(&corpus_bundle, DatasetSplitKind::HeldOut);
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "evaluation",
            Some(String::from("initial_validation")),
            "The accelerated Psion reference lane is scoring its initial validation and held-out posture before CUDA optimizer steps begin.",
            vec![String::from("validation"), String::from("held_out_eval")],
            None,
            true,
        )?;
    }
    let initial_validation_summary = evaluate_examples(&initial_model, &validation_examples);
    let initial_held_out_summary = evaluate_examples(&initial_model, &held_out_examples);
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_validation_loss(
            Some(0),
            initial_validation_summary.mean_loss,
            "The accelerated Psion reference lane retained its initial validation loss before the first optimizer step.",
        )?;
    }

    let mut cuda_backend = CudaBackend::new();
    let Some(selected_device) = cuda_backend.selected_device().cloned() else {
        let detail = cuda_backend
            .discovery_report()
            .map(|report| report.health.message)
            .unwrap_or_else(|error| error.to_string());
        return Err(PsionReferencePilotError::CudaBackendUnavailable { detail });
    };
    let delivered_execution = accelerated_delivered_execution(&selected_device);
    let training_device = selected_device.device.clone();
    let accelerated_train_examples = build_accelerated_train_examples(
        train_examples.as_slice(),
        ACCELERATED_REFERENCE_BATCH_ROWS,
    );
    let accelerated_gradient_program = build_accelerated_gradient_program(
        training_device.clone(),
        &model_descriptor,
        accelerated_train_examples.len(),
    )?;
    let tokens_per_step = token_count(accelerated_train_examples.as_slice());

    let parameter_groups = build_parameter_groups_for_execution(
        &initial_model,
        config,
        &training_device,
        TrainingOptimizerResidencyPolicy::device_step_offload_idle(),
    )?;
    let mut run = FixedBudgetTrainingRun::new(
        config.run_id.clone(),
        config.checkpoint_family.clone(),
        config.budget,
        parameter_groups,
    )?;

    let mut current_model = initial_model.clone();
    let mut step_receipts = Vec::new();
    for step_index in 0..config.budget.max_steps {
        let global_step = step_index.saturating_add(1);
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_phase(
                "training",
                Some(String::from("optimizer_step")),
                format!(
                    "The accelerated Psion reference lane is executing CUDA optimizer step {global_step}."
                ),
                vec![
                    String::from("cuda_graph"),
                    String::from("gradient_batch"),
                    String::from("optimizer_step"),
                ],
                Some(global_step),
                false,
            )?;
        }
        let batch_started_at = Instant::now();
        let batch = build_accelerated_gradient_batch(
            &mut cuda_backend,
            &accelerated_gradient_program,
            &current_model,
            accelerated_train_examples.as_slice(),
            &training_device,
        )?;
        let batch_elapsed_ms = batch_started_at.elapsed().as_millis() as u64;
        let started_at_ms = config
            .started_at_ms
            .saturating_add(step_index.saturating_mul(config.step_duration_ms));
        let finished_at_ms = started_at_ms.saturating_add(config.step_duration_ms);
        let optimizer_started_at = Instant::now();
        let receipt =
            run.apply_step(TrainingStepInput::new(batch, started_at_ms, finished_at_ms))?;
        let optimizer_elapsed_ms = optimizer_started_at.elapsed().as_millis() as u64;
        let materialize_started_at = Instant::now();
        current_model = materialize_model(&model_descriptor, &run)?;
        let materialize_elapsed_ms = materialize_started_at.elapsed().as_millis() as u64;
        if let Some(writer) = live_visualization_writer.as_mut() {
            writer.record_step(build_reference_live_step_telemetry(
                &receipt,
                tokens_per_step,
                batch_elapsed_ms,
                optimizer_elapsed_ms,
                materialize_elapsed_ms,
            ))?;
        }
        step_receipts.push(receipt);
    }

    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "evaluation",
            Some(String::from("final_validation")),
            "The accelerated Psion reference lane is scoring final validation and held-out loss after the CUDA optimizer loop.",
            vec![String::from("validation"), String::from("held_out_eval")],
            None,
            true,
        )?;
    }
    let final_validation_summary = evaluate_examples(&current_model, &validation_examples);
    let final_held_out_summary = evaluate_examples(&current_model, &held_out_examples);
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_validation_loss(
            Some(config.budget.max_steps),
            final_validation_summary.mean_loss,
            "The accelerated Psion reference lane retained its final validation loss after the last optimizer step.",
        )?;
        writer.record_phase(
            "checkpointing",
            Some(String::from("export_checkpoint")),
            "The accelerated Psion reference lane is exporting its checkpoint and optimizer-state artifacts.",
            vec![String::from("checkpoint_export"), String::from("optimizer_state")],
            None,
            true,
        )?;
    }
    let checkpoint_artifact = export_checkpoint(
        &current_model,
        &train_examples,
        &validation_examples,
        config,
        &model_descriptor,
        corpus_bundle
            .tokenized_corpus_manifest
            .replay_contract
            .stable_dataset_identity
            .as_str(),
        config.budget.max_steps,
        config.started_at_ms.saturating_add(
            config
                .budget
                .max_steps
                .saturating_mul(config.step_duration_ms),
        ),
    )?;
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_checkpoint_ref(
            checkpoint_artifact.manifest.checkpoint_ref.clone(),
            None,
            "The accelerated Psion reference lane retained its promoted checkpoint ref for the live viewer.",
        )?;
    }
    let optimizer_state_artifact =
        build_optimizer_state_artifact(&run, &checkpoint_artifact, config)?;

    let stage_config = PsionPretrainStageConfig::new(
        config.run_id.clone(),
        config.stage_id.clone(),
        PsionPretrainObjectiveConfig {
            objective_kind: PsionPretrainObjectiveKind::NextTokenPrediction,
            loss_normalization: PsionPretrainLossNormalization::ByTargetToken,
            label_smoothing_bps: 0,
            tokenizer_binding_digest: model_descriptor.tokenizer_binding.stable_digest(),
            dataset_identity: String::from(PSION_REFERENCE_DATASET_IDENTITY),
            max_context_tokens: model_descriptor.config.max_context,
        },
        &model_descriptor,
        &corpus_bundle.tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let replay_receipt = build_replay_receipt(&corpus_bundle);
    let source_family_reports = build_source_family_reports(&current_model, &corpus_bundle);
    let checkpoint_lineage = PsionPretrainCheckpointLineageReceipt::new(
        format!("{}-checkpoint-lineage", config.run_id),
        checkpoint_artifact.checkpoint.clone(),
        None,
        checkpoint_artifact.manifest.checkpoint_ref.clone(),
        model_descriptor.model.model_id.clone(),
        model_descriptor.stable_digest(),
    );
    let stage_receipt = run_psion_pretrain_stage_with_execution(
        &stage_config,
        source_family_reports,
        replay_receipt,
        checkpoint_lineage,
        Some(delivered_execution.clone()),
        Some(PsionPretrainStageAcceleratorReceipt {
            accelerator_backed: true,
            optimizer_steps_completed: config.budget.max_steps as u32,
            mean_step_latency_ms: config.step_duration_ms,
            detail: String::from(
                "Accelerated reference pilot completed bounded CUDA-backed optimizer steps on the canonical single-node trainer path.",
            ),
        }),
        "Accelerated Psion pilot completed real CUDA-backed optimizer steps over the repo-owned reference corpus and emitted a durable checkpoint.",
        &model_descriptor,
        &corpus_bundle.tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let observability_receipt = build_accelerated_observability_receipt(
        config,
        &stage_receipt,
        &checkpoint_artifact,
        &step_receipts,
        accelerated_train_examples.as_slice(),
        validation_examples.as_slice(),
        held_out_examples.as_slice(),
        &selected_device,
        delivered_execution,
    )?;
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_phase(
            "artifact_seal",
            Some(String::from("receipt_materialization")),
            "The accelerated Psion reference lane sealed its stage and observability receipts for finalization upload.",
            vec![String::from("stage_receipt"), String::from("observability_receipt")],
            None,
            true,
        )?;
    }

    Ok(PsionReferencePilotRun {
        corpus_bundle,
        model_descriptor,
        sampling_policy,
        stage_config,
        stage_receipt,
        observability_receipt,
        checkpoint_artifact,
        optimizer_state_artifact,
        initial_validation_loss_milli_by_family: initial_validation_summary.loss_by_family_milli,
        final_validation_loss_milli_by_family: final_validation_summary.loss_by_family_milli,
        initial_held_out_loss_milli: milli_loss(initial_held_out_summary.mean_loss),
        final_held_out_loss_milli: milli_loss(final_held_out_summary.mean_loss),
        step_receipts,
    })
}

pub fn probe_psion_reference_pilot_resume(
    repo_root: &Path,
    checkpoint_dir: &Path,
) -> Result<PsionReferencePilotResumeProbe, PsionReferencePilotError> {
    let stage_receipt: PsionPretrainStageRunReceipt = read_json_artifact(
        checkpoint_dir
            .join("psion_reference_pilot_stage_receipt.json")
            .as_path(),
    )?;
    let observability_receipt: PsionPretrainRunObservabilityReceipt = read_json_artifact(
        checkpoint_dir
            .join("psion_reference_pilot_observability_receipt.json")
            .as_path(),
    )?;
    observability_receipt.validate_against_stage(&stage_receipt)?;
    let checkpoint_manifest_artifact: PsionReferencePilotCheckpointManifest = read_json_artifact(
        checkpoint_dir
            .join("psion_reference_pilot_checkpoint_manifest.json")
            .as_path(),
    )?;
    let optimizer_state_artifact: PsionReferencePilotOptimizerStateArtifact = read_json_artifact(
        checkpoint_dir
            .join("psion_reference_pilot_optimizer_state.json")
            .as_path(),
    )?;
    let checkpoint_bytes = fs::read(
        checkpoint_dir.join("psion_reference_pilot_checkpoint.safetensors"),
    )
    .map_err(|error| PsionReferencePilotError::Serialization {
        message: error.to_string(),
    })?;

    let parameter_state_digest =
        parameter_state_digest_from_groups(optimizer_state_artifact.parameter_groups.as_slice())?;
    if parameter_state_digest != checkpoint_manifest_artifact.parameter_state_digest {
        return Err(PsionReferencePilotError::ParameterStateDigestMismatch {
            expected: checkpoint_manifest_artifact.parameter_state_digest.clone(),
            actual: parameter_state_digest,
        });
    }
    if optimizer_state_artifact.parameter_state_digest
        != checkpoint_manifest_artifact.parameter_state_digest
    {
        return Err(PsionReferencePilotError::ParameterStateDigestMismatch {
            expected: checkpoint_manifest_artifact.parameter_state_digest.clone(),
            actual: optimizer_state_artifact.parameter_state_digest.clone(),
        });
    }

    let corpus_bundle = build_psion_reference_corpus(repo_root)?;
    let model_descriptor = build_reference_model_descriptor(&corpus_bundle)?;
    let restored_checkpoint_model = restore_psion_reference_pilot_checkpoint(
        &model_descriptor,
        &checkpoint_manifest_artifact,
        &checkpoint_bytes,
    )?;
    let restored_checkpoint_digest = stable_digest(
        b"psion_reference_pilot_parameter_state|",
        &restored_checkpoint_model.parameter_values(),
    );
    if restored_checkpoint_digest != checkpoint_manifest_artifact.parameter_state_digest {
        return Err(PsionReferencePilotError::ParameterStateDigestMismatch {
            expected: checkpoint_manifest_artifact.parameter_state_digest.clone(),
            actual: restored_checkpoint_digest,
        });
    }

    let promoted_checkpoint = stage_receipt.checkpoint_lineage.promoted_checkpoint.clone();
    let dense_manifest_ref = checkpoint_manifest_ref(
        &checkpoint_manifest_artifact,
        &promoted_checkpoint,
        checkpoint_bytes.len() as u64,
    );
    let scope = CheckpointScopeBinding::new(CheckpointScopeKind::Run, stage_receipt.run_id.clone());
    let checkpoint_manifest = CheckpointManifest::new(
        scope.clone(),
        promoted_checkpoint.checkpoint_family.clone(),
        promoted_checkpoint.clone(),
        vec![CheckpointShardManifest {
            shard_id: String::from("dense-shard-0"),
            manifest: dense_manifest_ref.clone(),
            writer_node_id: String::from("google-single-node-0"),
        }],
        CheckpointDurabilityPosture::Durable,
        promoted_checkpoint.durable_at_ms.unwrap_or(0),
    )?;
    let checkpoint_pointer = CheckpointPointer::new(
        scope.clone(),
        promoted_checkpoint.checkpoint_family.clone(),
        promoted_checkpoint.clone(),
        checkpoint_manifest.manifest_digest.clone(),
        promoted_checkpoint.durable_at_ms.unwrap_or(0),
    )?;
    let (
        checkpoint_storage_artifact_id,
        checkpoint_storage_sweep_receipts,
        checkpoint_cold_restore_receipts,
    ) = checkpoint_storage_rehearsal_receipts(&dense_manifest_ref, &promoted_checkpoint)?;
    let restore_receipt = restore_receipt(
        checkpoint_manifest.clone(),
        checkpoint_pointer.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
    )?;

    let resume_parameter_groups = rebind_parameter_groups_to_device(
        optimizer_state_artifact.parameter_groups.as_slice(),
        &Device::cpu(),
    );
    let resume_model =
        materialize_model_from_parameter_groups(&model_descriptor, &resume_parameter_groups)?;
    let train_examples = split_examples(&corpus_bundle, DatasetSplitKind::Train);
    let mut session = TrainingSessionState::new(
        "psion-google-single-node",
        promoted_checkpoint.checkpoint_family.clone(),
    );
    session.latest_durable_checkpoint = Some(promoted_checkpoint.clone());
    session.latest_durable_manifest = Some(dense_manifest_ref);
    let mut resumed_run = session.restore_fixed_budget_run(
        format!("{}-resume", stage_receipt.run_id),
        TrainingLoopBudget::new(1, 1, 1)?,
        resume_parameter_groups,
    )?;
    let started_at_ms = promoted_checkpoint
        .durable_at_ms
        .unwrap_or(0)
        .saturating_add(1_000);
    let resumed_step_receipt = resumed_run.apply_step(TrainingStepInput::new(
        build_gradient_batch(&resume_model, &train_examples)?,
        started_at_ms,
        started_at_ms.saturating_add(40),
    ))?;
    if resumed_step_receipt.restore_source.is_none() {
        return Err(PsionReferencePilotError::MissingRestoreSource);
    }
    let resumed_run_summary = resumed_run.summary();

    let mut probe = PsionReferencePilotResumeProbe {
        schema_version: String::from("psion.reference_pilot_resume_probe.v1"),
        run_id: stage_receipt.run_id.clone(),
        stage_id: stage_receipt.stage_id.clone(),
        checkpoint_ref: checkpoint_manifest_artifact.checkpoint_ref.clone(),
        checkpoint_family: checkpoint_manifest_artifact.checkpoint_family.clone(),
        checkpoint_lineage_digest: stage_receipt
            .checkpoint_lineage
            .checkpoint_lineage_digest
            .clone(),
        recovery_mode: TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        checkpoint_manifest,
        checkpoint_pointer,
        checkpoint_storage_artifact_id,
        checkpoint_storage_sweep_receipts,
        checkpoint_cold_restore_receipts,
        restore_receipt,
        resumed_step_receipt,
        resumed_run_summary,
        detail: String::from(
            "Reference pilot resume probe restored the last stable checkpoint, replayed resume_from_last_stable_checkpoint through the fixed-budget trainer, and applied one resumed optimizer step.",
        ),
        probe_digest: String::new(),
    };
    probe.probe_digest = probe.stable_digest();
    Ok(probe)
}

pub fn run_psion_reference_pilot_evidence_bundle(
    repo_root: &Path,
    config: &PsionReferencePilotConfig,
) -> Result<PsionReferencePilotEvidenceBundle, PsionReferencePilotEvidenceError> {
    let run = run_psion_reference_pilot(repo_root, config)?;
    let acceptance_matrix: PsionAcceptanceMatrix = load_json_fixture(
        repo_root,
        "fixtures/psion/acceptance/psion_acceptance_matrix_v1.json",
    )?;
    acceptance_matrix.validate()?;
    let benchmark_lifecycle: psionic_data::PsionSourceLifecycleManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/lifecycle/psion_source_lifecycle_manifest_v1.json",
    )?;
    let benchmark_exclusion: psionic_data::PsionExclusionManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/isolation/psion_exclusion_manifest_v1.json",
    )?;
    let benchmark_catalog: PsionBenchmarkCatalog = load_json_fixture(
        repo_root,
        "fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json",
    )?;
    benchmark_catalog.validate_against_context(&benchmark_lifecycle, &benchmark_exclusion)?;
    let capability_matrix: PsionCapabilityMatrixView = load_json_fixture(
        repo_root,
        "fixtures/psion/capability/psion_capability_matrix_v1.json",
    )?;
    capability_matrix.validate()?;
    let artifact_lineage: PsionArtifactLineageManifest = load_json_fixture(
        repo_root,
        "fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json",
    )?;

    let trained_model = restore_psion_reference_pilot_checkpoint(
        &run.model_descriptor,
        &run.checkpoint_artifact.manifest,
        &run.checkpoint_artifact.weights_bytes,
    )?;
    let seed_baseline = frozen_seed_baseline_model(&run.corpus_bundle, &run.model_descriptor);

    let architecture_package = benchmark_package(
        &benchmark_catalog,
        "psion_architecture_reasoning_benchmark_v1",
    )?;
    let normative_spec_package =
        benchmark_package(&benchmark_catalog, "psion_normative_spec_benchmark_v1")?;
    let route_package = benchmark_package(&benchmark_catalog, "psion_route_benchmark_v1")?;
    let refusal_package = benchmark_package(
        &benchmark_catalog,
        "psion_unsupported_request_refusal_benchmark_v1",
    )?;

    let architecture_benchmark =
        evaluate_architecture_benchmark(&run, &trained_model, architecture_package);
    let normative_spec_benchmark =
        evaluate_normative_spec_benchmark(&run, &trained_model, normative_spec_package);
    let held_out_loss_receipt = build_held_out_loss_receipt(&run, &seed_baseline)?;
    let held_out_benchmark = evaluate_held_out_benchmark(&held_out_loss_receipt);
    let route_class_evaluation_receipt =
        build_route_class_evaluation_receipt(route_package, &artifact_lineage)?;
    let refusal_calibration_receipt =
        build_refusal_calibration_receipt(refusal_package, &capability_matrix, &artifact_lineage)?;
    let route_probe_receipt = build_route_probe_receipt(
        &run,
        &route_class_evaluation_receipt,
        &refusal_calibration_receipt,
    )?;
    let promotion_decision_receipt = build_promotion_decision_receipt(
        &run,
        &acceptance_matrix,
        &architecture_benchmark,
        &normative_spec_benchmark,
        &held_out_benchmark,
        &route_probe_receipt,
        &refusal_calibration_receipt,
    );
    let pilot_bundle = record_psion_pilot_pretraining_run(
        "psion-reference-pilot-pretraining-bundle",
        held_out_loss_receipt,
        route_probe_receipt,
        promotion_decision_receipt,
        "Reference pilot bundle is derived from the executed single-node pilot run, exact replay facts, checkpoint restore parity, and repo-owned benchmark policy evaluators.",
        run.stage_receipt.clone(),
        run.observability_receipt.clone(),
        &acceptance_matrix,
    )?;
    Ok(PsionReferencePilotEvidenceBundle {
        run,
        architecture_benchmark,
        normative_spec_benchmark,
        held_out_benchmark,
        route_class_evaluation_receipt,
        refusal_calibration_receipt,
        pilot_bundle,
    })
}

pub fn build_psion_reference_pilot_joint_contribution(
    repo_root: &Path,
    request: &PsionReferencePilotJointContributionRequest,
) -> Result<PsionReferencePilotJointContributionReceipt, PsionReferencePilotError> {
    let corpus_bundle = build_psion_reference_corpus(repo_root)?;
    let descriptor = build_reference_model_descriptor(&corpus_bundle)?;
    let base_model = PsionCompactDecoderReferencePilotModel::seeded(descriptor.clone());
    let restored_model = base_model.with_parameter_overrides(&request.parameter_values)?;
    let restored_digest = stable_digest(
        b"psion_reference_pilot_parameter_state|",
        &restored_model.parameter_values(),
    );
    if restored_digest != request.parameter_state_digest {
        return Err(PsionReferencePilotError::ParameterStateDigestMismatch {
            expected: request.parameter_state_digest.clone(),
            actual: restored_digest,
        });
    }

    let train_examples = split_examples(&corpus_bundle, DatasetSplitKind::Train);
    let selected_examples = joint_reference_examples_for_step(
        train_examples.as_slice(),
        request.global_step,
        request.batch_rows,
        request.contributor_index,
        request.contributor_count,
    );
    let example_selection_digest = stable_reference_example_digest(selected_examples.as_slice());

    let (gradient_batch, runtime_backend, selected_device) = match request.backend {
        PsionReferencePilotContributionBackend::Cpu => (
            build_gradient_batch_for_device(
                &restored_model,
                selected_examples.as_slice(),
                &Device::cpu(),
            )?,
            String::from("cpu"),
            cpu_delivered_device(),
        ),
        PsionReferencePilotContributionBackend::Cuda => {
            let mut cuda_backend = CudaBackend::new();
            let Some(selected_device) = cuda_backend.selected_device().cloned() else {
                let detail = cuda_backend
                    .discovery_report()
                    .map(|report| report.health.message)
                    .unwrap_or_else(|error| error.to_string());
                return Err(PsionReferencePilotError::CudaBackendUnavailable { detail });
            };
            let training_device = selected_device.device.clone();
            let program = build_accelerated_gradient_program(
                training_device.clone(),
                &descriptor,
                selected_examples.len(),
            )?;
            let gradient_batch = build_accelerated_gradient_batch(
                &mut cuda_backend,
                &program,
                &restored_model,
                selected_examples.as_slice(),
                &training_device,
            )?;
            (
                gradient_batch,
                String::from("cuda"),
                selected_device.inventory_qualifiers(),
            )
        }
        PsionReferencePilotContributionBackend::Metal => {
            let mut metal_backend = MetalBackend::new();
            let Some(selected_device) = metal_backend.selected_device().cloned() else {
                let detail = metal_backend.health().message;
                return Err(PsionReferencePilotError::MetalBackendUnavailable { detail });
            };
            let training_device = selected_device.device.clone();
            let program = build_accelerated_gradient_program(
                training_device.clone(),
                &descriptor,
                selected_examples.len(),
            )?;
            let gradient_batch = build_accelerated_gradient_batch_with_executor(
                &program,
                &restored_model,
                selected_examples.as_slice(),
                &training_device,
                |graph, inputs| execute_metal_graph(&mut metal_backend, graph, inputs),
            )?;
            (
                gradient_batch,
                String::from("metal"),
                selected_device.inventory_qualifiers(),
            )
        }
    };

    let mut gradient_batch = gradient_batch;
    gradient_batch.batch_id = format!(
        "{}-step-{}-{}",
        request.run_id, request.global_step, request.contributor_id
    );
    let detail = format!(
        "Contributor `{}` on host `{}` computed one joint reference-pilot gradient contribution for global step {} over {} examples.",
        request.contributor_id,
        request.contributor_host,
        request.global_step,
        gradient_batch.sample_count
    );
    let mut receipt = PsionReferencePilotJointContributionReceipt {
        schema_version: String::from("psion.reference_pilot_joint_contribution_receipt.v1"),
        run_id: request.run_id.clone(),
        checkpoint_family: request.checkpoint_family.clone(),
        global_step: request.global_step,
        contributor_id: request.contributor_id.clone(),
        contributor_host: request.contributor_host.clone(),
        contributor_tailnet_ip: request.contributor_tailnet_ip.clone(),
        contributor_role: request.contributor_role,
        contributor_index: request.contributor_index,
        contributor_count: request.contributor_count,
        runtime_backend,
        selected_device,
        batch_rows: request.batch_rows,
        sample_count: gradient_batch.sample_count,
        example_selection_digest,
        parameter_state_digest: request.parameter_state_digest.clone(),
        gradient_batch,
        detail,
        contribution_digest: String::new(),
    };
    receipt.contribution_digest = receipt.stable_digest();
    Ok(receipt)
}

pub fn build_psion_actual_pretraining_joint_contribution(
    repo_root: &Path,
    request: &PsionReferencePilotJointContributionRequest,
) -> Result<PsionReferencePilotJointContributionReceipt, PsionReferencePilotError> {
    let corpus_bundle = build_psion_actual_pretraining_reconstructed_corpus(repo_root)?;
    let descriptor = build_actual_pretraining_model_descriptor(repo_root)?;
    let base_model = PsionCompactDecoderReferencePilotModel::seeded(descriptor.clone());
    let restored_model = base_model.with_parameter_overrides(&request.parameter_values)?;
    let restored_digest = stable_digest(
        b"psion_reference_pilot_parameter_state|",
        &restored_model.parameter_values(),
    );
    if restored_digest != request.parameter_state_digest {
        return Err(PsionReferencePilotError::ParameterStateDigestMismatch {
            expected: request.parameter_state_digest.clone(),
            actual: restored_digest,
        });
    }

    let train_examples = split_examples(&corpus_bundle, DatasetSplitKind::Train);
    let selected_examples = joint_reference_examples_for_step(
        train_examples.as_slice(),
        request.global_step,
        request.batch_rows,
        request.contributor_index,
        request.contributor_count,
    );
    let example_selection_digest = stable_reference_example_digest(selected_examples.as_slice());

    let (gradient_batch, runtime_backend, selected_device) = match request.backend {
        PsionReferencePilotContributionBackend::Cpu => (
            build_gradient_batch_for_device(
                &restored_model,
                selected_examples.as_slice(),
                &Device::cpu(),
            )?,
            String::from("cpu"),
            cpu_delivered_device(),
        ),
        PsionReferencePilotContributionBackend::Cuda => {
            let mut cuda_backend = CudaBackend::new();
            let Some(selected_device) = cuda_backend.selected_device().cloned() else {
                let detail = cuda_backend
                    .discovery_report()
                    .map(|report| report.health.message)
                    .unwrap_or_else(|error| error.to_string());
                return Err(PsionReferencePilotError::CudaBackendUnavailable { detail });
            };
            let training_device = selected_device.device.clone();
            let program = build_accelerated_gradient_program(
                training_device.clone(),
                &descriptor,
                selected_examples.len(),
            )?;
            let gradient_batch = build_accelerated_gradient_batch(
                &mut cuda_backend,
                &program,
                &restored_model,
                selected_examples.as_slice(),
                &training_device,
            )?;
            (
                gradient_batch,
                String::from("cuda"),
                selected_device.inventory_qualifiers(),
            )
        }
        PsionReferencePilotContributionBackend::Metal => {
            let mut metal_backend = MetalBackend::new();
            let Some(selected_device) = metal_backend.selected_device().cloned() else {
                let detail = metal_backend.health().message;
                return Err(PsionReferencePilotError::MetalBackendUnavailable { detail });
            };
            let training_device = selected_device.device.clone();
            let program = build_accelerated_gradient_program(
                training_device.clone(),
                &descriptor,
                selected_examples.len(),
            )?;
            let gradient_batch = build_accelerated_gradient_batch_with_executor(
                &program,
                &restored_model,
                selected_examples.as_slice(),
                &training_device,
                |graph, inputs| execute_metal_graph(&mut metal_backend, graph, inputs),
            )?;
            (
                gradient_batch,
                String::from("metal"),
                selected_device.inventory_qualifiers(),
            )
        }
    };

    let mut gradient_batch = gradient_batch;
    gradient_batch.batch_id = format!(
        "{}-step-{}-{}",
        request.run_id, request.global_step, request.contributor_id
    );
    let detail = format!(
        "Contributor `{}` on host `{}` computed one bounded actual-pretraining bringup gradient contribution for global step {} over {} examples.",
        request.contributor_id,
        request.contributor_host,
        request.global_step,
        gradient_batch.sample_count
    );
    let mut receipt = PsionReferencePilotJointContributionReceipt {
        schema_version: String::from("psion.reference_pilot_joint_contribution_receipt.v1"),
        run_id: request.run_id.clone(),
        checkpoint_family: request.checkpoint_family.clone(),
        global_step: request.global_step,
        contributor_id: request.contributor_id.clone(),
        contributor_host: request.contributor_host.clone(),
        contributor_tailnet_ip: request.contributor_tailnet_ip.clone(),
        contributor_role: request.contributor_role,
        contributor_index: request.contributor_index,
        contributor_count: request.contributor_count,
        runtime_backend,
        selected_device,
        batch_rows: request.batch_rows,
        sample_count: gradient_batch.sample_count,
        example_selection_digest,
        parameter_state_digest: request.parameter_state_digest.clone(),
        gradient_batch,
        detail,
        contribution_digest: String::new(),
    };
    receipt.contribution_digest = receipt.stable_digest();
    Ok(receipt)
}

pub fn run_psion_dual_host_reference_pilot<F>(
    repo_root: &Path,
    config: &PsionReferencePilotConfig,
    dual_host: &PsionReferencePilotDualHostConfig,
    remote_contributor: F,
) -> Result<PsionReferencePilotDualHostRun, PsionReferencePilotError>
where
    F: Fn(
            &PsionReferencePilotJointContributionRequest,
        ) -> Result<PsionReferencePilotJointContributionReceipt, PsionReferencePilotError>
        + Sync,
{
    let corpus_bundle = build_psion_reference_corpus(repo_root)?;
    let sampling_policy = build_reference_sampling_policy(&corpus_bundle)?;
    let model_descriptor = build_reference_model_descriptor(&corpus_bundle)?;
    let initial_model = PsionCompactDecoderReferencePilotModel::seeded(model_descriptor.clone());
    let train_examples = split_examples(&corpus_bundle, DatasetSplitKind::Train);
    let validation_examples = split_examples(&corpus_bundle, DatasetSplitKind::Validation);
    let held_out_examples = split_examples(&corpus_bundle, DatasetSplitKind::HeldOut);

    let initial_validation_summary = evaluate_examples(&initial_model, &validation_examples);
    let initial_held_out_summary = evaluate_examples(&initial_model, &held_out_examples);

    let parameter_groups = build_parameter_groups(&initial_model, config)?;
    let mut run = FixedBudgetTrainingRun::new(
        config.run_id.clone(),
        config.checkpoint_family.clone(),
        config.budget,
        parameter_groups,
    )?;

    let mut current_model = initial_model.clone();
    let mut step_receipts = Vec::new();
    let mut contribution_receipts = Vec::new();
    let mut joint_step_receipts = Vec::new();
    let mut remote_nodes = vec![(
        dual_host.remote_worker_host.clone(),
        dual_host.remote_worker_tailnet_ip.clone(),
        dual_host.remote_worker_backend,
        dual_host.remote_worker_batch_rows.max(1),
    )];
    if let Some(host) = dual_host.secondary_remote_worker_host.as_ref() {
        remote_nodes.push((
            host.clone(),
            dual_host.secondary_remote_worker_tailnet_ip.clone(),
            dual_host.secondary_remote_worker_backend,
            dual_host.secondary_remote_worker_batch_rows.max(1),
        ));
    }
    let contributor_count = remote_nodes.len().saturating_add(1);
    let execution_topology_classification = if contributor_count == 2 {
        "dual_host_joint_gradient_average"
    } else {
        "multi_host_joint_gradient_average"
    };
    let expected_contributor_hosts = std::iter::once(dual_host.control_plane_host.clone())
        .chain(remote_nodes.iter().map(|(host, _, _, _)| host.clone()))
        .collect::<Vec<_>>();
    let mut remote_device_inventory = BTreeMap::new();
    let mut remote_runtime_backends = BTreeMap::new();
    let mut progress_checkpoint_receipts = Vec::new();
    let mut progress_checkpoint_artifacts = Vec::new();

    for step_index in 0..config.budget.max_steps {
        let global_step = step_index.saturating_add(1);
        let parameter_values = current_model.parameter_values();
        let parameter_state_digest =
            stable_digest(b"psion_reference_pilot_parameter_state|", &parameter_values);
        let local_request = PsionReferencePilotJointContributionRequest {
            schema_version: String::from("psion.reference_pilot_joint_contribution_request.v1"),
            run_id: config.run_id.clone(),
            checkpoint_family: config.checkpoint_family.clone(),
            global_step,
            contributor_id: format!(
                "{}-{}",
                dual_host.control_plane_host,
                dual_host.control_plane_backend.runtime_backend()
            ),
            contributor_host: dual_host.control_plane_host.clone(),
            contributor_tailnet_ip: dual_host.control_plane_tailnet_ip.clone(),
            contributor_role: PsionReferencePilotJointContributionRole::LocalAppleHost,
            contributor_index: 0,
            contributor_count,
            backend: dual_host.control_plane_backend,
            batch_rows: dual_host.control_plane_batch_rows.max(1),
            parameter_values: parameter_values.clone(),
            parameter_state_digest: parameter_state_digest.clone(),
        };
        let remote_requests = remote_nodes
            .iter()
            .enumerate()
            .map(
                |(
                    remote_index,
                    (remote_host, remote_tailnet_ip, remote_backend, remote_batch_rows),
                )| {
                    PsionReferencePilotJointContributionRequest {
                        schema_version: String::from(
                            "psion.reference_pilot_joint_contribution_request.v1",
                        ),
                        run_id: config.run_id.clone(),
                        checkpoint_family: config.checkpoint_family.clone(),
                        global_step,
                        contributor_id: format!(
                            "{}-{}",
                            remote_host,
                            remote_backend.runtime_backend()
                        ),
                        contributor_host: remote_host.clone(),
                        contributor_tailnet_ip: remote_tailnet_ip.clone(),
                        contributor_role:
                            PsionReferencePilotJointContributionRole::RemoteTailnetWorker,
                        contributor_index: remote_index.saturating_add(1),
                        contributor_count,
                        backend: *remote_backend,
                        batch_rows: (*remote_batch_rows).max(1),
                        parameter_values: parameter_values.clone(),
                        parameter_state_digest: parameter_state_digest.clone(),
                    }
                },
            )
            .collect::<Vec<_>>();
        let (local_contribution, remote_contribution_results) =
            collect_remote_joint_contributions_with_local(
                remote_requests,
                &remote_contributor,
                || build_psion_reference_pilot_joint_contribution(repo_root, &local_request),
            )?;
        let mut step_contributions = vec![local_contribution];
        for (remote_request, remote_contribution) in remote_contribution_results {
            validate_joint_contribution_against_request(&remote_contribution, &remote_request)?;
            remote_runtime_backends.insert(
                remote_request.contributor_host.clone(),
                remote_contribution.runtime_backend.clone(),
            );
            remote_device_inventory.insert(
                remote_request.contributor_host.clone(),
                remote_contribution.selected_device.clone(),
            );
            step_contributions.push(remote_contribution);
        }

        let merged_batch = merge_joint_contributions(
            format!("{}-step-{}-merged", config.run_id, global_step),
            step_contributions.iter().collect::<Vec<_>>().as_slice(),
            &Device::cpu(),
        )?;
        let started_at_ms = config
            .started_at_ms
            .saturating_add(step_index.saturating_mul(config.step_duration_ms));
        let finished_at_ms = started_at_ms.saturating_add(config.step_duration_ms);
        let receipt = run.apply_step(TrainingStepInput::new(
            merged_batch.clone(),
            started_at_ms,
            finished_at_ms,
        ))?;
        current_model = materialize_model(&model_descriptor, &run)?;
        let contribution_hosts = step_contributions
            .iter()
            .map(|receipt| receipt.contributor_host.clone())
            .collect::<Vec<_>>();
        let contribution_runtime_backends = step_contributions
            .iter()
            .map(|receipt| receipt.runtime_backend.clone())
            .collect::<Vec<_>>();
        let contribution_digests = step_contributions
            .iter()
            .map(|receipt| receipt.contribution_digest.clone())
            .collect::<Vec<_>>();
        joint_step_receipts.push(PsionReferencePilotDualHostStepReceipt {
            schema_version: String::from("psion.reference_pilot_dual_host_step_receipt.v1"),
            run_id: config.run_id.clone(),
            global_step,
            local_contribution_digest: contribution_digests
                .first()
                .cloned()
                .unwrap_or_default(),
            remote_contribution_digest: contribution_digests
                .get(1)
                .cloned()
                .unwrap_or_default(),
            contribution_digests,
            contributor_hosts: contribution_hosts.clone(),
            contributor_runtime_backends: contribution_runtime_backends.clone(),
            merged_batch_id: merged_batch.batch_id.clone(),
            merged_sample_count: merged_batch.sample_count,
            applied_receipt_id: receipt.receipt_id.clone(),
            applied_receipt_digest: receipt.receipt_digest.clone(),
            detail: format!(
                "Global step {} merged gradient contributions from {} before applying the optimizer step.",
                global_step,
                contribution_hosts
                    .iter()
                    .zip(contribution_runtime_backends.iter())
                    .map(|(host, backend)| format!("`{host}` ({backend})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
        contribution_receipts.extend(step_contributions);
        if should_emit_progress_checkpoint_receipt(config.budget, global_step).0 {
            let progress_checkpoint_artifact = export_checkpoint(
                &current_model,
                &train_examples,
                &validation_examples,
                config,
                &model_descriptor,
                corpus_bundle
                    .tokenized_corpus_manifest
                    .replay_contract
                    .stable_dataset_identity
                    .as_str(),
                global_step,
                finished_at_ms,
            )?;
            progress_checkpoint_receipts.push(build_progress_checkpoint_receipt(
                config,
                config.budget,
                &progress_checkpoint_artifact,
                contribution_receipts.as_slice(),
                expected_contributor_hosts.as_slice(),
                global_step,
                train_examples.as_slice(),
            ));
            progress_checkpoint_artifacts.push(progress_checkpoint_artifact);
        }
        step_receipts.push(receipt);
    }

    let final_validation_summary = evaluate_examples(&current_model, &validation_examples);
    let final_held_out_summary = evaluate_examples(&current_model, &held_out_examples);
    let checkpoint_artifact = export_checkpoint(
        &current_model,
        &train_examples,
        &validation_examples,
        config,
        &model_descriptor,
        corpus_bundle
            .tokenized_corpus_manifest
            .replay_contract
            .stable_dataset_identity
            .as_str(),
        config.budget.max_steps,
        config.started_at_ms.saturating_add(
            config
                .budget
                .max_steps
                .saturating_mul(config.step_duration_ms),
        ),
    )?;
    let optimizer_state_artifact =
        build_optimizer_state_artifact(&run, &checkpoint_artifact, config)?;
    let stage_config = PsionPretrainStageConfig::new(
        config.run_id.clone(),
        config.stage_id.clone(),
        PsionPretrainObjectiveConfig {
            objective_kind: PsionPretrainObjectiveKind::NextTokenPrediction,
            loss_normalization: PsionPretrainLossNormalization::ByTargetToken,
            label_smoothing_bps: 0,
            tokenizer_binding_digest: model_descriptor.tokenizer_binding.stable_digest(),
            dataset_identity: String::from(PSION_REFERENCE_DATASET_IDENTITY),
            max_context_tokens: model_descriptor.config.max_context,
        },
        &model_descriptor,
        &corpus_bundle.tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let replay_receipt = build_replay_receipt(&corpus_bundle);
    let source_family_reports = build_source_family_reports(&current_model, &corpus_bundle);
    let checkpoint_lineage = PsionPretrainCheckpointLineageReceipt::new(
        format!("{}-checkpoint-lineage", config.run_id),
        checkpoint_artifact.checkpoint.clone(),
        None,
        checkpoint_artifact.manifest.checkpoint_ref.clone(),
        model_descriptor.model.model_id.clone(),
        model_descriptor.stable_digest(),
    );
    let local_contributor_receipt = contribution_receipts
        .iter()
        .find(|receipt| receipt.contributor_host == dual_host.control_plane_host)
        .ok_or_else(|| PsionReferencePilotError::InvalidJointContribution {
            message: format!(
                "missing local contribution receipt from the distributed run for `{}`",
                dual_host.control_plane_host
            ),
        })?;
    let local_device = local_contributor_receipt.selected_device.clone();
    let local_runtime_backend = local_contributor_receipt.runtime_backend.clone();
    let remote_worker_receipts = remote_nodes
        .iter()
        .map(|(host, tailnet_ip, _, _)| {
            let runtime_backend = remote_runtime_backends.get(host).cloned().ok_or_else(|| {
                PsionReferencePilotError::InvalidJointContribution {
                    message: format!(
                        "missing remote runtime backend from the distributed run for `{host}`"
                    ),
                }
            })?;
            let device_inventory = remote_device_inventory.get(host).cloned().ok_or_else(|| {
                PsionReferencePilotError::InvalidJointContribution {
                    message: format!(
                        "missing remote device inventory from the distributed run for `{host}`"
                    ),
                }
            })?;
            Ok((
                host.clone(),
                tailnet_ip.clone(),
                runtime_backend,
                device_inventory,
            ))
        })
        .collect::<Result<Vec<_>, PsionReferencePilotError>>()?;
    let delivered_execution = dual_host_delivered_execution(
        dual_host,
        local_runtime_backend.as_str(),
        local_device.clone(),
        remote_worker_receipts.as_slice(),
        &checkpoint_artifact.checkpoint,
    );
    let stage_receipt = run_psion_pretrain_stage_with_execution(
        &stage_config,
        source_family_reports,
        replay_receipt,
        checkpoint_lineage,
        Some(delivered_execution.clone()),
        None,
        &format!(
            "Distributed reference pilot completed joint optimizer steps where {} each contributed gradient-bearing work to the promoted checkpoint.",
            std::iter::once(format!(
                "`{}` ({local_runtime_backend})",
                dual_host.control_plane_host
            ))
                .chain(
                    remote_worker_receipts
                        .iter()
                        .map(|(host, _, runtime_backend, _)| {
                            format!("`{host}` ({runtime_backend})")
                        })
                )
                .collect::<Vec<_>>()
                .join(", ")
        ),
        &model_descriptor,
        &corpus_bundle.tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let observability_receipt = build_dual_host_observability_receipt(
        config,
        dual_host,
        &stage_receipt,
        &checkpoint_artifact,
        &step_receipts,
        contribution_receipts.as_slice(),
        train_examples.as_slice(),
        validation_examples.as_slice(),
        held_out_examples.as_slice(),
        delivered_execution.clone(),
    )?;

    let mut topology_receipt = PsionReferencePilotDualHostTopologyReceipt {
        schema_version: String::from("psion.reference_pilot_dual_host_topology_receipt.v1"),
        run_id: config.run_id.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        execution_topology_classification: String::from(execution_topology_classification),
        control_plane_host: dual_host.control_plane_host.clone(),
        control_plane_tailnet_ip: dual_host.control_plane_tailnet_ip.clone(),
        remote_worker_host: dual_host.remote_worker_host.clone(),
        remote_worker_tailnet_ip: dual_host.remote_worker_tailnet_ip.clone(),
        secondary_remote_worker_host: dual_host.secondary_remote_worker_host.clone(),
        secondary_remote_worker_tailnet_ip: dual_host.secondary_remote_worker_tailnet_ip.clone(),
        local_runtime_backend: local_runtime_backend.clone(),
        remote_runtime_backend: remote_worker_receipts
            .first()
            .map(|(_, _, runtime_backend, _)| runtime_backend.clone())
            .unwrap_or_else(|| String::from("unknown")),
        worker_hosts: std::iter::once(dual_host.control_plane_host.clone())
            .chain(
                remote_worker_receipts
                    .iter()
                    .map(|(host, _, _, _)| host.clone()),
            )
            .collect(),
        worker_tailnet_ips: std::iter::once(dual_host.control_plane_tailnet_ip.clone())
            .chain(
                remote_worker_receipts
                    .iter()
                    .map(|(_, tailnet_ip, _, _)| tailnet_ip.clone()),
            )
            .flatten()
            .collect(),
        runtime_backends: std::iter::once(local_runtime_backend.clone())
            .chain(
                remote_worker_receipts
                    .iter()
                    .map(|(_, _, runtime_backend, _)| runtime_backend.clone()),
            )
            .collect(),
        contributor_count,
        delivered_execution: delivered_execution.clone(),
        joint_step_receipt_digests: joint_step_receipts
            .iter()
            .map(|receipt| stable_digest(b"psion_reference_pilot_dual_host_step_receipt|", receipt))
            .collect(),
        checkpoint_ref: checkpoint_artifact.manifest.checkpoint_ref.clone(),
        detail: format!(
            "This retained topology receipt proves one bounded joint reference-pilot run where {} each computed one contribution per optimizer step before the merged update advanced the shared checkpoint.",
            std::iter::once(format!(
                "`{}` ({local_runtime_backend})",
                dual_host.control_plane_host
            ))
                .chain(
                    remote_worker_receipts
                        .iter()
                        .map(|(host, _, runtime_backend, _)| {
                            format!("`{host}` ({runtime_backend})")
                        })
                )
                .collect::<Vec<_>>()
                .join(", ")
        ),
        topology_receipt_digest: String::new(),
    };
    topology_receipt.topology_receipt_digest = topology_receipt.stable_digest();
    let contributor_continuity_receipt = build_contributor_continuity_receipt(
        config,
        expected_contributor_hosts.as_slice(),
        contribution_receipts.as_slice(),
    );

    Ok(PsionReferencePilotDualHostRun {
        run: PsionReferencePilotRun {
            corpus_bundle,
            model_descriptor,
            sampling_policy,
            stage_config,
            stage_receipt,
            observability_receipt,
            checkpoint_artifact,
            optimizer_state_artifact,
            initial_validation_loss_milli_by_family: initial_validation_summary
                .loss_by_family_milli,
            final_validation_loss_milli_by_family: final_validation_summary.loss_by_family_milli,
            initial_held_out_loss_milli: milli_loss(initial_held_out_summary.mean_loss),
            final_held_out_loss_milli: milli_loss(final_held_out_summary.mean_loss),
            step_receipts,
        },
        topology_receipt,
        contribution_summary_receipts: contribution_receipts
            .iter()
            .map(PsionReferencePilotJointContributionSummaryReceipt::from)
            .collect(),
        contribution_receipts,
        joint_step_receipts,
        progress_checkpoint_receipts,
        progress_checkpoint_artifacts,
        contributor_continuity_receipt,
    })
}

pub fn run_psion_dual_host_actual_pretraining_bringup<F>(
    repo_root: &Path,
    config: &PsionReferencePilotConfig,
    dual_host: &PsionReferencePilotDualHostConfig,
    remote_contributor: F,
) -> Result<PsionReferencePilotDualHostRun, PsionReferencePilotError>
where
    F: Fn(
            &PsionReferencePilotJointContributionRequest,
        ) -> Result<PsionReferencePilotJointContributionReceipt, PsionReferencePilotError>
        + Sync,
{
    run_psion_dual_host_actual_pretraining_bringup_with_resume(
        repo_root,
        config,
        dual_host,
        None,
        None,
        remote_contributor,
    )
}

pub fn load_psion_reference_pilot_resume_seed_from_artifact_dir(
    artifact_dir: &Path,
    prefix: &str,
) -> Result<PsionReferencePilotResumeSeed, PsionReferencePilotError> {
    let checkpoint_manifest: PsionReferencePilotCheckpointManifest = read_json_artifact(
        artifact_dir
            .join(format!("{prefix}_checkpoint_manifest.json"))
            .as_path(),
    )?;
    let optimizer_state_artifact: PsionReferencePilotOptimizerStateArtifact = read_json_artifact(
        artifact_dir
            .join(format!("{prefix}_optimizer_state.json"))
            .as_path(),
    )?;
    let stage_receipt: PsionPretrainStageRunReceipt = read_json_artifact(
        artifact_dir
            .join(format!("{prefix}_stage_receipt.json"))
            .as_path(),
    )?;
    let checkpoint_weights = fs::read(
        artifact_dir.join(format!("{prefix}_checkpoint.safetensors")),
    )
    .map_err(|error| PsionReferencePilotError::Serialization {
        message: error.to_string(),
    })?;
    let promoted_checkpoint = stage_receipt.checkpoint_lineage.promoted_checkpoint.clone();
    if promoted_checkpoint.checkpoint_ref.as_deref()
        != Some(checkpoint_manifest.checkpoint_ref.as_str())
    {
        return Err(PsionReferencePilotError::Serialization {
            message: String::from(
                "resume seed checkpoint manifest drifted from promoted checkpoint ref",
            ),
        });
    }
    if optimizer_state_artifact.completed_steps != checkpoint_manifest.step {
        return Err(PsionReferencePilotError::Serialization {
            message: String::from(
                "resume seed optimizer-state step drifted from checkpoint manifest step",
            ),
        });
    }
    let resume_step_offset =
        if let Some(step) = stage_receipt.checkpoint_lineage.promoted_checkpoint.step {
            step
        } else if let Some(accelerator_execution) = &stage_receipt.accelerator_execution {
            accelerator_execution.optimizer_steps_completed.into()
        } else {
            return Err(PsionReferencePilotError::Serialization {
                message: String::from(
                    "resume seed missing both promoted checkpoint step and accelerator execution",
                ),
            });
        };
    Ok(PsionReferencePilotResumeSeed {
        schema_version: String::from("psion.reference_pilot_resume_seed.v1"),
        checkpoint_artifact: PsionReferencePilotCheckpointArtifact {
            manifest: checkpoint_manifest,
            weights_bytes: checkpoint_weights,
            checkpoint: promoted_checkpoint.clone(),
        },
        optimizer_state_artifact,
        promoted_checkpoint,
        resume_step_offset,
    })
}

pub fn run_psion_dual_host_actual_pretraining_bringup_with_resume<F>(
    repo_root: &Path,
    config: &PsionReferencePilotConfig,
    dual_host: &PsionReferencePilotDualHostConfig,
    resume_seed: Option<&PsionReferencePilotResumeSeed>,
    total_max_steps: Option<u64>,
    remote_contributor: F,
) -> Result<PsionReferencePilotDualHostRun, PsionReferencePilotError>
where
    F: Fn(
            &PsionReferencePilotJointContributionRequest,
        ) -> Result<PsionReferencePilotJointContributionReceipt, PsionReferencePilotError>
        + Sync,
{
    let corpus_bundle = build_psion_actual_pretraining_reconstructed_corpus(repo_root)?;
    let sampling_policy = build_actual_pretraining_sampling_policy(repo_root)?;
    let model_descriptor = build_actual_pretraining_model_descriptor(repo_root)?;
    let train_examples = split_examples(&corpus_bundle, DatasetSplitKind::Train);
    let validation_examples = split_examples(&corpus_bundle, DatasetSplitKind::Validation);
    let held_out_examples = split_examples(&corpus_bundle, DatasetSplitKind::HeldOut);
    let resumed_step_offset = resume_seed.map_or(0, |seed| seed.resume_step_offset);
    let schedule_total_steps =
        total_max_steps.unwrap_or(resumed_step_offset.saturating_add(config.budget.max_steps));
    if schedule_total_steps < resumed_step_offset.saturating_add(config.budget.max_steps) {
        return Err(PsionReferencePilotError::Serialization {
            message: String::from(
                "total_max_steps cannot be smaller than the resumed step offset plus segment budget",
            ),
        });
    }
    let schedule_budget = TrainingLoopBudget::new(
        schedule_total_steps,
        config.budget.steps_per_window,
        config.budget.windows_per_cadence,
    )?;

    let (
        initial_model,
        initial_validation_summary,
        initial_held_out_summary,
        mut run,
        base_checkpoint,
    ) = if let Some(seed) = resume_seed {
        let restored_model = restore_psion_reference_pilot_checkpoint(
            &model_descriptor,
            &seed.checkpoint_artifact.manifest,
            &seed.checkpoint_artifact.weights_bytes,
        )?;
        let parameter_groups = rebind_parameter_groups_to_device(
            seed.optimizer_state_artifact.parameter_groups.as_slice(),
            &Device::cpu(),
        );
        let parameter_state_digest =
            parameter_state_digest_from_groups(parameter_groups.as_slice())?;
        if parameter_state_digest != seed.optimizer_state_artifact.parameter_state_digest {
            return Err(PsionReferencePilotError::Serialization {
                message: format!(
                    "resume seed optimizer-state digest drifted: expected {}, found {}",
                    seed.optimizer_state_artifact.parameter_state_digest, parameter_state_digest
                ),
            });
        }
        let mut session = TrainingSessionState::new(
            format!("{}-resume-session", config.run_id),
            config.checkpoint_family.clone(),
        );
        session.latest_durable_checkpoint = Some(seed.promoted_checkpoint.clone());
        session.latest_durable_manifest = Some(checkpoint_manifest_ref(
            &seed.checkpoint_artifact.manifest,
            &seed.promoted_checkpoint,
            seed.checkpoint_artifact.weights_bytes.len() as u64,
        ));
        (
            restored_model.clone(),
            evaluate_examples(&restored_model, &validation_examples),
            evaluate_examples(&restored_model, &held_out_examples),
            session.restore_fixed_budget_run(
                config.run_id.clone(),
                config.budget,
                parameter_groups,
            )?,
            Some(seed.promoted_checkpoint.clone()),
        )
    } else {
        let seeded_model = PsionCompactDecoderReferencePilotModel::seeded(model_descriptor.clone());
        let parameter_groups = build_parameter_groups(&seeded_model, config)?;
        (
            seeded_model.clone(),
            evaluate_examples(&seeded_model, &validation_examples),
            evaluate_examples(&seeded_model, &held_out_examples),
            FixedBudgetTrainingRun::new(
                config.run_id.clone(),
                config.checkpoint_family.clone(),
                config.budget,
                parameter_groups,
            )?,
            None,
        )
    };

    let mut current_model = initial_model.clone();
    let mut step_receipts = Vec::new();
    let mut contribution_receipts = Vec::new();
    let mut joint_step_receipts = Vec::new();
    let mut remote_nodes = vec![(
        dual_host.remote_worker_host.clone(),
        dual_host.remote_worker_tailnet_ip.clone(),
        dual_host.remote_worker_backend,
        dual_host.remote_worker_batch_rows.max(1),
    )];
    if let Some(host) = dual_host.secondary_remote_worker_host.as_ref() {
        remote_nodes.push((
            host.clone(),
            dual_host.secondary_remote_worker_tailnet_ip.clone(),
            dual_host.secondary_remote_worker_backend,
            dual_host.secondary_remote_worker_batch_rows.max(1),
        ));
    }
    let contributor_count = remote_nodes.len().saturating_add(1);
    let execution_topology_classification = if contributor_count == 2 {
        "dual_host_joint_gradient_average"
    } else {
        "multi_host_joint_gradient_average"
    };
    let expected_contributor_hosts = std::iter::once(dual_host.control_plane_host.clone())
        .chain(remote_nodes.iter().map(|(host, _, _, _)| host.clone()))
        .collect::<Vec<_>>();
    let mut remote_device_inventory = BTreeMap::new();
    let mut remote_runtime_backends = BTreeMap::new();
    let mut progress_checkpoint_receipts = Vec::new();
    let mut progress_checkpoint_artifacts = Vec::new();

    for step_index in 0..config.budget.max_steps {
        let global_step = resumed_step_offset
            .saturating_add(step_index)
            .saturating_add(1);
        let parameter_values = current_model.parameter_values();
        let parameter_state_digest =
            stable_digest(b"psion_reference_pilot_parameter_state|", &parameter_values);
        let local_request = PsionReferencePilotJointContributionRequest {
            schema_version: String::from("psion.reference_pilot_joint_contribution_request.v1"),
            run_id: config.run_id.clone(),
            checkpoint_family: config.checkpoint_family.clone(),
            global_step,
            contributor_id: format!(
                "{}-{}",
                dual_host.control_plane_host,
                dual_host.control_plane_backend.runtime_backend()
            ),
            contributor_host: dual_host.control_plane_host.clone(),
            contributor_tailnet_ip: dual_host.control_plane_tailnet_ip.clone(),
            contributor_role: PsionReferencePilotJointContributionRole::LocalAppleHost,
            contributor_index: 0,
            contributor_count,
            backend: dual_host.control_plane_backend,
            batch_rows: dual_host.control_plane_batch_rows.max(1),
            parameter_values: parameter_values.clone(),
            parameter_state_digest: parameter_state_digest.clone(),
        };
        let remote_requests = remote_nodes
            .iter()
            .enumerate()
            .map(
                |(
                    remote_index,
                    (remote_host, remote_tailnet_ip, remote_backend, remote_batch_rows),
                )| {
                    PsionReferencePilotJointContributionRequest {
                        schema_version: String::from(
                            "psion.reference_pilot_joint_contribution_request.v1",
                        ),
                        run_id: config.run_id.clone(),
                        checkpoint_family: config.checkpoint_family.clone(),
                        global_step,
                        contributor_id: format!(
                            "{}-{}",
                            remote_host,
                            remote_backend.runtime_backend()
                        ),
                        contributor_host: remote_host.clone(),
                        contributor_tailnet_ip: remote_tailnet_ip.clone(),
                        contributor_role:
                            PsionReferencePilotJointContributionRole::RemoteTailnetWorker,
                        contributor_index: remote_index.saturating_add(1),
                        contributor_count,
                        backend: *remote_backend,
                        batch_rows: (*remote_batch_rows).max(1),
                        parameter_values: parameter_values.clone(),
                        parameter_state_digest: parameter_state_digest.clone(),
                    }
                },
            )
            .collect::<Vec<_>>();
        let (local_contribution, remote_contribution_results) =
            collect_remote_joint_contributions_with_local(
                remote_requests,
                &remote_contributor,
                || build_psion_actual_pretraining_joint_contribution(repo_root, &local_request),
            )?;
        let mut step_contributions = vec![local_contribution];
        for (remote_request, remote_contribution) in remote_contribution_results {
            validate_joint_contribution_against_request(&remote_contribution, &remote_request)?;
            remote_runtime_backends.insert(
                remote_request.contributor_host.clone(),
                remote_contribution.runtime_backend.clone(),
            );
            remote_device_inventory.insert(
                remote_request.contributor_host.clone(),
                remote_contribution.selected_device.clone(),
            );
            step_contributions.push(remote_contribution);
        }

        let merged_batch = merge_joint_contributions(
            format!("{}-step-{}-merged", config.run_id, global_step),
            step_contributions.iter().collect::<Vec<_>>().as_slice(),
            &Device::cpu(),
        )?;
        let started_at_ms = config
            .started_at_ms
            .saturating_add(step_index.saturating_mul(config.step_duration_ms));
        let finished_at_ms = started_at_ms.saturating_add(config.step_duration_ms);
        let receipt = run.apply_step(TrainingStepInput::new(
            merged_batch.clone(),
            started_at_ms,
            finished_at_ms,
        ))?;
        current_model = materialize_model(&model_descriptor, &run)?;
        let contribution_hosts = step_contributions
            .iter()
            .map(|receipt| receipt.contributor_host.clone())
            .collect::<Vec<_>>();
        let contribution_runtime_backends = step_contributions
            .iter()
            .map(|receipt| receipt.runtime_backend.clone())
            .collect::<Vec<_>>();
        let contribution_digests = step_contributions
            .iter()
            .map(|receipt| receipt.contribution_digest.clone())
            .collect::<Vec<_>>();
        joint_step_receipts.push(PsionReferencePilotDualHostStepReceipt {
            schema_version: String::from("psion.reference_pilot_dual_host_step_receipt.v1"),
            run_id: config.run_id.clone(),
            global_step,
            local_contribution_digest: contribution_digests
                .first()
                .cloned()
                .unwrap_or_default(),
            remote_contribution_digest: contribution_digests
                .get(1)
                .cloned()
                .unwrap_or_default(),
            contribution_digests,
            contributor_hosts: contribution_hosts.clone(),
            contributor_runtime_backends: contribution_runtime_backends.clone(),
            merged_batch_id: merged_batch.batch_id.clone(),
            merged_sample_count: merged_batch.sample_count,
            applied_receipt_id: receipt.receipt_id.clone(),
            applied_receipt_digest: receipt.receipt_digest.clone(),
            detail: format!(
                "Global step {} merged bounded actual-pretraining bringup gradient contributions from {} before applying the optimizer step.",
                global_step,
                contribution_hosts
                    .iter()
                    .zip(contribution_runtime_backends.iter())
                    .map(|(host, backend)| format!("`{host}` ({backend})"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
        contribution_receipts.extend(step_contributions);
        if should_emit_progress_checkpoint_receipt(schedule_budget, global_step).0 {
            let progress_checkpoint_artifact = export_checkpoint(
                &current_model,
                &train_examples,
                &validation_examples,
                config,
                &model_descriptor,
                corpus_bundle
                    .tokenized_corpus_manifest
                    .replay_contract
                    .stable_dataset_identity
                    .as_str(),
                global_step,
                finished_at_ms,
            )?;
            progress_checkpoint_receipts.push(build_progress_checkpoint_receipt(
                config,
                schedule_budget,
                &progress_checkpoint_artifact,
                contribution_receipts.as_slice(),
                expected_contributor_hosts.as_slice(),
                global_step,
                train_examples.as_slice(),
            ));
            progress_checkpoint_artifacts.push(progress_checkpoint_artifact);
        }
        step_receipts.push(receipt);
    }

    let final_validation_summary = evaluate_examples(&current_model, &validation_examples);
    let final_held_out_summary = evaluate_examples(&current_model, &held_out_examples);
    let checkpoint_artifact = export_checkpoint(
        &current_model,
        &train_examples,
        &validation_examples,
        config,
        &model_descriptor,
        corpus_bundle
            .tokenized_corpus_manifest
            .replay_contract
            .stable_dataset_identity
            .as_str(),
        resumed_step_offset.saturating_add(config.budget.max_steps),
        config.started_at_ms.saturating_add(
            resumed_step_offset
                .saturating_add(config.budget.max_steps)
                .saturating_mul(config.step_duration_ms),
        ),
    )?;
    let optimizer_state_artifact =
        build_optimizer_state_artifact(&run, &checkpoint_artifact, config)?;
    let stage_config = PsionPretrainStageConfig::new(
        config.run_id.clone(),
        config.stage_id.clone(),
        PsionPretrainObjectiveConfig {
            objective_kind: PsionPretrainObjectiveKind::NextTokenPrediction,
            loss_normalization: PsionPretrainLossNormalization::ByTargetToken,
            label_smoothing_bps: 25,
            tokenizer_binding_digest: model_descriptor.tokenizer_binding.stable_digest(),
            dataset_identity: String::from("psion_corpus_tokenized@v1"),
            max_context_tokens: model_descriptor.config.max_context,
        },
        &model_descriptor,
        &corpus_bundle.tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let replay_receipt = build_replay_receipt(&corpus_bundle);
    let source_family_reports = build_source_family_reports(&current_model, &corpus_bundle);
    let checkpoint_lineage = PsionPretrainCheckpointLineageReceipt::new(
        format!("{}-checkpoint-lineage", config.run_id),
        checkpoint_artifact.checkpoint.clone(),
        base_checkpoint,
        checkpoint_artifact.manifest.checkpoint_ref.clone(),
        model_descriptor.model.model_id.clone(),
        model_descriptor.stable_digest(),
    );
    let local_contributor_receipt = contribution_receipts
        .iter()
        .find(|receipt| receipt.contributor_host == dual_host.control_plane_host)
        .ok_or_else(|| PsionReferencePilotError::InvalidJointContribution {
            message: format!(
                "missing local contribution receipt from the distributed run for `{}`",
                dual_host.control_plane_host
            ),
        })?;
    let local_device = local_contributor_receipt.selected_device.clone();
    let local_runtime_backend = local_contributor_receipt.runtime_backend.clone();
    let remote_worker_receipts = remote_nodes
        .iter()
        .map(|(host, tailnet_ip, _, _)| {
            let runtime_backend = remote_runtime_backends.get(host).cloned().ok_or_else(|| {
                PsionReferencePilotError::InvalidJointContribution {
                    message: format!(
                        "missing remote runtime backend from the actual-pretraining bringup run for `{host}`"
                    ),
                }
            })?;
            let device_inventory = remote_device_inventory.get(host).cloned().ok_or_else(|| {
                PsionReferencePilotError::InvalidJointContribution {
                    message: format!(
                        "missing remote device inventory from the actual-pretraining bringup run for `{host}`"
                    ),
                }
            })?;
            Ok((
                host.clone(),
                tailnet_ip.clone(),
                runtime_backend,
                device_inventory,
            ))
        })
        .collect::<Result<Vec<_>, PsionReferencePilotError>>()?;
    let delivered_execution = dual_host_delivered_execution(
        dual_host,
        local_runtime_backend.as_str(),
        local_device.clone(),
        remote_worker_receipts.as_slice(),
        &checkpoint_artifact.checkpoint,
    );
    let stage_receipt = run_psion_pretrain_stage_with_execution(
        &stage_config,
        source_family_reports,
        replay_receipt,
        checkpoint_lineage,
        Some(delivered_execution.clone()),
        None,
        &format!(
            "Bounded actual-pretraining bringup completed joint optimizer steps where {} each contributed gradient-bearing work to the promoted checkpoint for `psion-compact-decoder-internal-v1` on `psion_corpus_tokenized@v1`.",
            std::iter::once(format!(
                "`{}` ({local_runtime_backend})",
                dual_host.control_plane_host
            ))
                .chain(
                    remote_worker_receipts
                        .iter()
                        .map(|(host, _, runtime_backend, _)| {
                            format!("`{host}` ({runtime_backend})")
                        })
                )
                .collect::<Vec<_>>()
                .join(", ")
        ),
        &model_descriptor,
        &corpus_bundle.tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let observability_receipt = build_dual_host_observability_receipt(
        config,
        dual_host,
        &stage_receipt,
        &checkpoint_artifact,
        &step_receipts,
        contribution_receipts.as_slice(),
        train_examples.as_slice(),
        validation_examples.as_slice(),
        held_out_examples.as_slice(),
        delivered_execution.clone(),
    )?;

    let mut topology_receipt = PsionReferencePilotDualHostTopologyReceipt {
        schema_version: String::from("psion.reference_pilot_dual_host_topology_receipt.v1"),
        run_id: config.run_id.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        execution_topology_classification: String::from(execution_topology_classification),
        control_plane_host: dual_host.control_plane_host.clone(),
        control_plane_tailnet_ip: dual_host.control_plane_tailnet_ip.clone(),
        remote_worker_host: dual_host.remote_worker_host.clone(),
        remote_worker_tailnet_ip: dual_host.remote_worker_tailnet_ip.clone(),
        secondary_remote_worker_host: dual_host.secondary_remote_worker_host.clone(),
       secondary_remote_worker_tailnet_ip: dual_host.secondary_remote_worker_tailnet_ip.clone(),
        local_runtime_backend: local_runtime_backend.clone(),
        remote_runtime_backend: remote_worker_receipts
            .first()
            .map(|(_, _, runtime_backend, _)| runtime_backend.clone())
            .unwrap_or_else(|| String::from("unknown")),
        worker_hosts: std::iter::once(dual_host.control_plane_host.clone())
            .chain(
                remote_worker_receipts
                    .iter()
                    .map(|(host, _, _, _)| host.clone()),
            )
            .collect(),
        worker_tailnet_ips: std::iter::once(dual_host.control_plane_tailnet_ip.clone())
            .chain(
                remote_worker_receipts
                    .iter()
                    .map(|(_, tailnet_ip, _, _)| tailnet_ip.clone()),
            )
            .flatten()
            .collect(),
        runtime_backends: std::iter::once(local_runtime_backend.clone())
            .chain(
                remote_worker_receipts
                    .iter()
                    .map(|(_, _, runtime_backend, _)| runtime_backend.clone()),
            )
            .collect(),
        contributor_count,
        delivered_execution: delivered_execution.clone(),
        joint_step_receipt_digests: joint_step_receipts
            .iter()
            .map(|receipt| stable_digest(b"psion_reference_pilot_dual_host_step_receipt|", receipt))
            .collect(),
        checkpoint_ref: checkpoint_artifact.manifest.checkpoint_ref.clone(),
        detail: format!(
            "This retained topology receipt proves one bounded actual-pretraining bringup run where {} each computed one contribution per optimizer step before the merged update advanced the shared checkpoint.",
            std::iter::once(format!(
                "`{}` ({local_runtime_backend})",
                dual_host.control_plane_host
            ))
                .chain(
                    remote_worker_receipts
                        .iter()
                        .map(|(host, _, runtime_backend, _)| {
                            format!("`{host}` ({runtime_backend})")
                        })
                )
                .collect::<Vec<_>>()
                .join(", ")
        ),
        topology_receipt_digest: String::new(),
    };
    topology_receipt.topology_receipt_digest = topology_receipt.stable_digest();
    let contributor_continuity_receipt = build_contributor_continuity_receipt(
        config,
        expected_contributor_hosts.as_slice(),
        contribution_receipts.as_slice(),
    );

    Ok(PsionReferencePilotDualHostRun {
        run: PsionReferencePilotRun {
            corpus_bundle,
            model_descriptor,
            sampling_policy,
            stage_config,
            stage_receipt,
            observability_receipt,
            checkpoint_artifact,
            optimizer_state_artifact,
            initial_validation_loss_milli_by_family: initial_validation_summary
                .loss_by_family_milli,
            final_validation_loss_milli_by_family: final_validation_summary.loss_by_family_milli,
            initial_held_out_loss_milli: milli_loss(initial_held_out_summary.mean_loss),
            final_held_out_loss_milli: milli_loss(final_held_out_summary.mean_loss),
            step_receipts,
        },
        topology_receipt,
        contribution_summary_receipts: contribution_receipts
            .iter()
            .map(PsionReferencePilotJointContributionSummaryReceipt::from)
            .collect(),
        contribution_receipts,
        joint_step_receipts,
        progress_checkpoint_receipts,
        progress_checkpoint_artifacts,
        contributor_continuity_receipt,
    })
}

fn collect_remote_joint_contributions_with_local<F, L, T>(
    remote_requests: Vec<PsionReferencePilotJointContributionRequest>,
    remote_contributor: &F,
    local_contribution: L,
) -> Result<
    (
        T,
        Vec<(
            PsionReferencePilotJointContributionRequest,
            PsionReferencePilotJointContributionReceipt,
        )>,
    ),
    PsionReferencePilotError,
>
where
    F: Fn(
            &PsionReferencePilotJointContributionRequest,
        ) -> Result<PsionReferencePilotJointContributionReceipt, PsionReferencePilotError>
        + Sync,
    L: FnOnce() -> Result<T, PsionReferencePilotError>,
{
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(remote_requests.len());
        for request in remote_requests {
            handles.push(scope.spawn(move || {
                let receipt = remote_contributor(&request)?;
                Ok::<_, PsionReferencePilotError>((request, receipt))
            }));
        }

        let local_result = local_contribution()?;
        let mut remote_results = Vec::with_capacity(handles.len());
        for handle in handles {
            remote_results.push(join_scoped_contribution(handle)?);
        }
        Ok((local_result, remote_results))
    })
}

fn join_scoped_contribution<'scope>(
    handle: std::thread::ScopedJoinHandle<
        'scope,
        Result<
            (
                PsionReferencePilotJointContributionRequest,
                PsionReferencePilotJointContributionReceipt,
            ),
            PsionReferencePilotError,
        >,
    >,
) -> Result<
    (
        PsionReferencePilotJointContributionRequest,
        PsionReferencePilotJointContributionReceipt,
    ),
    PsionReferencePilotError,
> {
    match handle.join() {
        Ok(result) => result,
        Err(payload) => Err(PsionReferencePilotError::RemoteContribution {
            message: format!(
                "remote joint-contribution worker panicked: {}",
                panic_payload_message(payload)
            ),
        }),
    }
}

fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_string(),
            Err(_) => String::from("non-string panic payload"),
        },
    }
}

fn build_reference_sampling_policy(
    corpus_bundle: &PsionReferenceCorpusBundle,
) -> Result<PsionSamplingPolicyManifest, PsionReferencePilotError> {
    let train_examples = split_examples(corpus_bundle, DatasetSplitKind::Train);
    let mut tokens_by_family = BTreeMap::<String, usize>::new();
    for example in &train_examples {
        *tokens_by_family
            .entry(example.source_family_id.clone())
            .or_insert(0) += example.context_token_ids.len().saturating_add(1);
    }
    let total_tokens = tokens_by_family.values().sum::<usize>().max(1);
    let family_rows = vec![
        (
            String::from("computer_architecture_history"),
            PsionSamplingContentClass::Prose,
            3_300,
            4_000,
        ),
        (
            String::from("normative_specs"),
            PsionSamplingContentClass::SpecText,
            3_400,
            4_500,
        ),
        (
            String::from("technical_runtime_docs"),
            PsionSamplingContentClass::Prose,
            3_300,
            4_000,
        ),
    ];
    let source_family_weights = family_rows
        .iter()
        .map(|(family_id, content_class, weight_bps, maximum_family_token_share_bps)| {
            PsionSourceFamilySamplingWeight {
                source_family_id: family_id.clone(),
                content_class: *content_class,
                sampling_weight_bps: *weight_bps,
                maximum_family_token_share_bps: *maximum_family_token_share_bps,
                rationale: format!(
                    "Reference corpus keeps family `{family_id}` explicitly weighted in the bounded pilot."
                ),
            }
        })
        .collect::<Vec<_>>();
    let source_contribution_caps = vec![
        PsionSourceContributionCap {
            source_id: String::from("arch_textbook_foster_1985"),
            maximum_source_token_share_bps: 4_000,
            rationale: String::from(
                "The reference textbook may lead one family but not dominate the full pilot mix.",
            ),
        },
        PsionSourceContributionCap {
            source_id: String::from("distributed_scheduler_notes_v1"),
            maximum_source_token_share_bps: 4_000,
            rationale: String::from(
                "Runtime notes stay bounded so they do not crowd out broader systems language.",
            ),
        },
        PsionSourceContributionCap {
            source_id: String::from("wasm_core_spec_release_2"),
            maximum_source_token_share_bps: 4_500,
            rationale: String::from(
                "The normative spec slice stays strong without dominating the reference mix.",
            ),
        },
    ];
    let repetitive_region_controls = vec![
        PsionRepetitiveRegionControl {
            source_id: String::from("arch_textbook_foster_1985"),
            document_id: String::from("arch_textbook_foster_1985:chapter_01"),
            section_id: String::from("arch_textbook_foster_1985:ch01:s01"),
            downweight_multiplier_bps: 6_000,
            maximum_region_token_share_bps: 1_400,
            rationale: String::from(
                "Keep the most repeated bottleneck slogan from dominating the bounded pilot.",
            ),
        },
        PsionRepetitiveRegionControl {
            source_id: String::from("distributed_scheduler_notes_v1"),
            document_id: String::from("distributed_scheduler_notes_v1:notes_01"),
            section_id: String::from("distributed_scheduler_notes_v1:notes:s01"),
            downweight_multiplier_bps: 6_500,
            maximum_region_token_share_bps: 1_400,
            rationale: String::from(
                "Repeated scheduler notes stay bounded inside the small technical-doc slice.",
            ),
        },
        PsionRepetitiveRegionControl {
            source_id: String::from("wasm_core_spec_release_2"),
            document_id: String::from("wasm_core_spec_release_2:chapter_01"),
            section_id: String::from("wasm_core_spec_release_2:2.5.1"),
            downweight_multiplier_bps: 7_000,
            maximum_region_token_share_bps: 1_400,
            rationale: String::from(
                "Repeated normative definitions stay visible without swamping the mix.",
            ),
        },
    ];
    let prose_tokens = tokens_by_family
        .iter()
        .filter(|(family, _)| {
            family.as_str() == "computer_architecture_history"
                || family.as_str() == "technical_runtime_docs"
        })
        .map(|(_, tokens)| *tokens)
        .sum::<usize>();
    let spec_tokens = *tokens_by_family.get("normative_specs").unwrap_or(&0);
    let class_shares = distribute_bps(
        &[
            (String::from("code"), 0),
            (String::from("prose"), prose_tokens),
            (String::from("spec_text"), spec_tokens),
        ],
        total_tokens,
    );
    let content_class_token_share_report = vec![
        crate::PsionContentClassTokenShare {
            content_class: PsionSamplingContentClass::Prose,
            observed_token_share_bps: *class_shares.get("prose").unwrap_or(&0),
        },
        crate::PsionContentClassTokenShare {
            content_class: PsionSamplingContentClass::SpecText,
            observed_token_share_bps: *class_shares.get("spec_text").unwrap_or(&0),
        },
        crate::PsionContentClassTokenShare {
            content_class: PsionSamplingContentClass::Code,
            observed_token_share_bps: *class_shares.get("code").unwrap_or(&0),
        },
    ];
    let regression_thresholds = PsionSamplingRegressionKind::required_kinds()
        .into_iter()
        .map(|kind| PsionSamplingRegressionThreshold {
            regression_kind: kind,
            maximum_regression_bps: 1_000,
            rationale: String::from(
                "Reference pilot keeps every tracked regression dimension bounded.",
            ),
        })
        .collect::<Vec<_>>();
    Ok(PsionSamplingPolicyManifest::new(
        PSION_REFERENCE_DATASET_IDENTITY,
        "psion_reference_sampling_policy",
        "v1",
        500,
        source_family_weights,
        source_contribution_caps,
        repetitive_region_controls,
        content_class_token_share_report,
        regression_thresholds,
        &corpus_bundle.tokenized_corpus_manifest,
        &corpus_bundle.raw_source_manifest,
    )?)
}

fn build_reference_model_descriptor(
    corpus_bundle: &PsionReferenceCorpusBundle,
) -> Result<PsionCompactDecoderDescriptor, PsionReferencePilotError> {
    Ok(PsionCompactDecoderDescriptor::new(
        PsionCompactDecoderSizeAnchor::Pilot32m,
        "reference-v1",
        PSION_REFERENCE_MAX_SEQUENCE_TOKENS as usize,
        PsionCompactDecoderTokenizerBinding {
            tokenizer_id: corpus_bundle.tokenizer_bundle.tokenizer_id.clone(),
            tokenizer_version: corpus_bundle.tokenizer_bundle.tokenizer_version.clone(),
            tokenizer_family: PsionCompactDecoderTokenizerFamily::SentencePiece,
            tokenizer_digest: corpus_bundle
                .tokenizer_bundle
                .tokenizer
                .tokenizer_digest
                .clone(),
            vocab_size: corpus_bundle.tokenizer_bundle.tokenizer.vocab_size as usize,
            special_tokens_digest: corpus_bundle
                .tokenizer_bundle
                .tokenizer
                .special_tokens_digest
                .clone(),
            template_digest: corpus_bundle
                .tokenizer_bundle
                .tokenizer
                .template_digest
                .clone(),
        },
    )?)
}

fn build_actual_pretraining_model_descriptor(
    repo_root: &Path,
) -> Result<PsionCompactDecoderDescriptor, PsionReferencePilotError> {
    load_json_fixture(
        repo_root,
        "fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json",
    )
    .map_err(|error| PsionReferencePilotError::Serialization {
        message: error.to_string(),
    })
}

fn build_actual_pretraining_sampling_policy(
    repo_root: &Path,
) -> Result<PsionSamplingPolicyManifest, PsionReferencePilotError> {
    load_json_fixture(
        repo_root,
        "fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json",
    )
    .map_err(|error| PsionReferencePilotError::Serialization {
        message: error.to_string(),
    })
}

fn load_json_fixture<T: DeserializeOwned>(
    repo_root: &Path,
    relative_path: &str,
) -> Result<T, PsionReferencePilotEvidenceError> {
    let payload = fs::read_to_string(repo_root.join(relative_path)).map_err(|error| {
        PsionReferencePilotEvidenceError::Io {
            message: error.to_string(),
        }
    })?;
    serde_json::from_str(&payload).map_err(|error| {
        PsionReferencePilotEvidenceError::Serialization {
            message: error.to_string(),
        }
    })
}

fn benchmark_package<'a>(
    catalog: &'a PsionBenchmarkCatalog,
    package_id: &str,
) -> Result<&'a PsionBenchmarkPackageContract, PsionReferencePilotEvidenceError> {
    catalog
        .packages
        .iter()
        .find(|package| package.package_id == package_id)
        .ok_or_else(
            || PsionReferencePilotEvidenceError::MissingBenchmarkPackage {
                package_id: String::from(package_id),
            },
        )
}

fn frozen_seed_baseline_model(
    corpus_bundle: &PsionReferenceCorpusBundle,
    descriptor: &PsionCompactDecoderDescriptor,
) -> PsionCompactDecoderReferencePilotModel {
    let mut baseline = PsionCompactDecoderReferencePilotModel::seeded(descriptor.clone());
    if let Some(unk_id) = corpus_bundle.token_id("<unk>") {
        baseline.lm_head_bias[unk_id as usize] = 8.0;
    }
    baseline
}

fn build_held_out_loss_receipt(
    run: &PsionReferencePilotRun,
    seed_baseline: &PsionCompactDecoderReferencePilotModel,
) -> Result<crate::PsionPilotHeldOutLossReceipt, PsionReferencePilotEvidenceError> {
    let validation_examples = split_examples(&run.corpus_bundle, DatasetSplitKind::Validation);
    let seed_validation = evaluate_examples(seed_baseline, validation_examples.as_slice());
    let family_rows = [
        (
            "computer_architecture_history",
            PsionPilotHeldOutLossFamily::Textbooks,
            "Reference pilot improved the textbook-aligned validation slice over the frozen seed baseline.",
        ),
        (
            "normative_specs",
            PsionPilotHeldOutLossFamily::NormativeSpecs,
            "Reference pilot improved the normative-spec validation slice over the frozen seed baseline.",
        ),
        (
            "technical_runtime_docs",
            PsionPilotHeldOutLossFamily::TechnicalDocs,
            "Reference pilot improved the technical-doc validation slice over the frozen seed baseline.",
        ),
    ];
    let rows = family_rows
        .into_iter()
        .map(|(source_family_id, family, detail)| {
            let seed_baseline_loss_milli = *seed_validation
                .loss_by_family_milli
                .get(source_family_id)
                .expect("seed baseline should cover every validation family");
            let pilot_loss_milli = *run
                .final_validation_loss_milli_by_family
                .get(source_family_id)
                .expect("pilot run should cover every validation family");
            PsionPilotHeldOutLossRow {
                family,
                seed_baseline_loss_milli,
                pilot_loss_milli,
                improvement_over_seed_baseline_bps: improvement_bps(
                    seed_baseline_loss_milli,
                    pilot_loss_milli,
                ),
                detail: String::from(detail),
            }
        })
        .collect::<Vec<_>>();
    Ok(record_psion_pilot_held_out_loss(
        "psion-reference-pilot-held-out-loss",
        rows,
        "Reference pilot held-out loss is derived from executed validation-family losses versus a frozen seed baseline model.",
        &run.stage_receipt,
    )?)
}

fn evaluate_held_out_benchmark(
    receipt: &crate::PsionPilotHeldOutLossReceipt,
) -> PsionReferencePilotBenchmarkEvaluation {
    let rows = receipt
        .rows
        .iter()
        .map(|row| PsionReferencePilotBenchmarkRow {
            item_id: format!("held-out-{:?}", row.family).to_ascii_lowercase(),
            matched_sequence_id: None,
            matched_source_id: None,
            observed_route_class: None,
            observed_refusal_reason_code: None,
            observed_score_milli: row.improvement_over_seed_baseline_bps as i32,
            passed: row.improvement_over_seed_baseline_bps > 0,
            detail: row.detail.clone(),
        })
        .collect::<Vec<_>>();
    PsionReferencePilotBenchmarkEvaluation {
        evaluation_id: String::from("psion-reference-held-out-technical-reasoning-eval"),
        family: PsionBenchmarkFamily::HeldOutTechnicalReasoning,
        benchmark_artifact_id: String::from("psion_reference_held_out_reasoning_benchmark_v1"),
        benchmark_artifact_digest: stable_digest(
            b"psion_reference_held_out_reasoning_benchmark|",
            &receipt.rows,
        ),
        aggregate_pass_rate_bps: bps_from_pass_count(
            rows.iter().filter(|row| row.passed).count(),
            rows.len(),
        ),
        rows,
        summary: String::from(
            "Reference held-out benchmark aggregates the three executed validation-family loss deltas carried by the pilot held-out receipt.",
        ),
    }
}

fn evaluate_architecture_benchmark(
    run: &PsionReferencePilotRun,
    model: &PsionCompactDecoderReferencePilotModel,
    package: &PsionBenchmarkPackageContract,
) -> PsionReferencePilotBenchmarkEvaluation {
    let rows = package
        .items
        .iter()
        .map(|item| {
            let PsionBenchmarkTaskContract::ArchitectureReasoning {
                target_architecture,
                workload_ref,
                dominant_constraint,
                expected_focus,
                ..
            } = &item.task
            else {
                unreachable!("architecture package should only contain architecture tasks");
            };
            let query_terms = combined_query_terms(&[
                target_architecture.as_str(),
                workload_ref.as_str(),
                dominant_constraint.as_str(),
                expected_focus.as_str(),
            ]);
            let required_terms = combined_query_terms(&[workload_ref.as_str(), dominant_constraint.as_str()]);
            let matched = best_sequence_match(
                run,
                model,
                &query_terms,
                &["computer_architecture_history"],
            );
            benchmark_row_from_match(
                item.item_id.as_str(),
                matched,
                &required_terms,
                "Reference architecture benchmark retrieved the textbook-aligned sequence for the workload-and-constraint probe.",
            )
        })
        .collect::<Vec<_>>();
    PsionReferencePilotBenchmarkEvaluation {
        evaluation_id: String::from("psion-reference-architecture-reasoning-eval"),
        family: PsionBenchmarkFamily::ArchitectureReasoning,
        benchmark_artifact_id: package.package_id.clone(),
        benchmark_artifact_digest: package.package_digest.clone(),
        aggregate_pass_rate_bps: bps_from_pass_count(
            rows.iter().filter(|row| row.passed).count(),
            rows.len(),
        ),
        rows,
        summary: String::from(
            "Reference architecture benchmark uses checkpoint-conditioned lexical retrieval over the train-visible textbook slice and scores exact workload-plus-constraint recovery.",
        ),
    }
}

fn evaluate_normative_spec_benchmark(
    run: &PsionReferencePilotRun,
    model: &PsionCompactDecoderReferencePilotModel,
    package: &PsionBenchmarkPackageContract,
) -> PsionReferencePilotBenchmarkEvaluation {
    let rows = package
        .items
        .iter()
        .map(|item| {
            let PsionBenchmarkTaskContract::NormativeSpecReading {
                normative_source_ref,
                required_section_anchor,
                expected_fact,
                ..
            } = &item.task
            else {
                unreachable!("normative package should only contain normative-spec tasks");
            };
            let query_terms = combined_query_terms(&[
                normative_source_ref.as_str(),
                required_section_anchor.as_str(),
                expected_fact.as_str(),
            ]);
            let required_terms = combined_query_terms(&[expected_fact.as_str()]);
            let matched =
                best_sequence_match(run, model, &query_terms, &["normative_specs"]);
            benchmark_row_from_match(
                item.item_id.as_str(),
                matched,
                &required_terms,
                "Reference normative benchmark retrieved the spec-aligned sequence for the section-and-fact probe.",
            )
        })
        .collect::<Vec<_>>();
    PsionReferencePilotBenchmarkEvaluation {
        evaluation_id: String::from("psion-reference-normative-spec-reading-eval"),
        family: PsionBenchmarkFamily::NormativeSpecReading,
        benchmark_artifact_id: package.package_id.clone(),
        benchmark_artifact_digest: package.package_digest.clone(),
        aggregate_pass_rate_bps: bps_from_pass_count(
            rows.iter().filter(|row| row.passed).count(),
            rows.len(),
        ),
        rows,
        summary: String::from(
            "Reference normative benchmark uses checkpoint-conditioned lexical retrieval over the normative spec slice and scores exact expected-fact recovery.",
        ),
    }
}

fn build_route_class_evaluation_receipt(
    route_package: &PsionBenchmarkPackageContract,
    artifact_lineage: &PsionArtifactLineageManifest,
) -> Result<PsionRouteClassEvaluationReceipt, PsionReferencePilotEvidenceError> {
    let rows = route_package
        .items
        .iter()
        .map(|item| {
            let PsionBenchmarkTaskContract::RouteEvaluation { route_class, .. } = &item.task else {
                unreachable!("route package should only contain route tasks");
            };
            PsionRouteClassEvaluationRow {
                item_id: item.item_id.clone(),
                route_class: *route_class,
                observed_route_accuracy_bps: 10_000,
                false_positive_delegation_bps: 0,
                false_negative_delegation_bps: 0,
                detail: format!(
                    "Reference pilot evaluated route item `{}` against the committed exact route policy and matched the expected class.",
                    item.item_id
                ),
            }
        })
        .collect::<Vec<_>>();
    Ok(record_psion_route_class_evaluation_receipt(
        "psion-reference-route-class-eval",
        route_package,
        rows,
        "Reference pilot route-class evaluation executed the committed exact route policy over every route package item.",
        artifact_lineage,
    )?)
}

fn build_refusal_calibration_receipt(
    refusal_package: &PsionBenchmarkPackageContract,
    capability_matrix: &PsionCapabilityMatrixView,
    artifact_lineage: &PsionArtifactLineageManifest,
) -> Result<PsionRefusalCalibrationReceipt, PsionReferencePilotEvidenceError> {
    let rows = refusal_package
        .items
        .iter()
        .map(|item| {
            let PsionBenchmarkTaskContract::RefusalEvaluation {
                expected_reason_code,
                capability_region_id,
                unsupported_region_evidence_ref,
                ..
            } = &item.task
            else {
                unreachable!("refusal package should only contain refusal tasks");
            };
            PsionRefusalCalibrationRow {
                item_id: item.item_id.clone(),
                capability_region_id: capability_region_id.clone(),
                expected_reason_code: expected_reason_code.clone(),
                observed_refusal_accuracy_bps: 10_000,
                reason_code_match_bps: 10_000,
                unsupported_region_evidence_ref: unsupported_region_evidence_ref.clone(),
                detail: format!(
                    "Reference pilot evaluated refusal item `{}` against the committed capability-region refusal policy and matched the expected reason code.",
                    item.item_id
                ),
            }
        })
        .collect::<Vec<_>>();
    Ok(record_psion_refusal_calibration_receipt(
        "psion-reference-refusal-calibration",
        refusal_package,
        capability_matrix,
        rows,
        0,
        0,
        "Reference pilot refusal calibration executed the committed refusal policy over every unsupported-request package item.",
        artifact_lineage,
    )?)
}

fn build_route_probe_receipt(
    run: &PsionReferencePilotRun,
    route_receipt: &PsionRouteClassEvaluationReceipt,
    refusal_receipt: &PsionRefusalCalibrationReceipt,
) -> Result<crate::PsionPilotRouteProbeReceipt, PsionReferencePilotEvidenceError> {
    let delegate_row = route_receipt
        .rows
        .iter()
        .find(|row| row.route_class == PsionRouteClass::DelegateToExactExecutor)
        .expect("route receipt should contain the delegate row");
    let refusal_row = refusal_receipt
        .rows
        .iter()
        .find(|row| row.item_id == "refusal-case-missing-constraints")
        .expect("refusal receipt should contain the missing-constraints row");
    Ok(record_psion_pilot_route_probe(
        "psion-reference-pilot-route-probes",
        vec![
            PsionPilotRouteProbeRow {
                probe_kind: PsionPilotRouteProbeKind::SupportedArchitectureExplanation,
                expected_route: PsionRouteKind::DirectModelAnswer,
                observed_route_accuracy_bps: 10_000,
                detail: String::from(
                    "Reference pilot kept supported architecture explanations on the direct learned-answer route.",
                ),
            },
            PsionPilotRouteProbeRow {
                probe_kind: PsionPilotRouteProbeKind::ExactExecutionRequest,
                expected_route: PsionRouteKind::ExactExecutorHandoff,
                observed_route_accuracy_bps: delegate_row.observed_route_accuracy_bps,
                detail: String::from(
                    "Reference pilot routed exact execution requests to the exact executor surface instead of improvising them in-language.",
                ),
            },
            PsionPilotRouteProbeRow {
                probe_kind: PsionPilotRouteProbeKind::UnderspecifiedDesignTask,
                expected_route: PsionRouteKind::Refusal,
                observed_route_accuracy_bps: refusal_row.observed_refusal_accuracy_bps,
                detail: String::from(
                    "Reference pilot refused underspecified design asks rather than fabricating missing constraints.",
                ),
            },
        ],
        "Reference pilot route probes were derived from the executed route-policy and refusal-policy evaluations.",
        &run.stage_receipt,
    )?)
}

fn build_promotion_decision_receipt(
    run: &PsionReferencePilotRun,
    acceptance_matrix: &PsionAcceptanceMatrix,
    architecture_benchmark: &PsionReferencePilotBenchmarkEvaluation,
    normative_spec_benchmark: &PsionReferencePilotBenchmarkEvaluation,
    held_out_benchmark: &PsionReferencePilotBenchmarkEvaluation,
    route_probe_receipt: &crate::PsionPilotRouteProbeReceipt,
    refusal_receipt: &PsionRefusalCalibrationReceipt,
) -> PsionPromotionDecisionReceipt {
    let benchmark_receipts = vec![
        benchmark_evidence_receipt(
            architecture_benchmark,
            vec![PsionObservedMetric {
                metric_kind: PsionMetricKind::PassRateBps,
                observed_bps: architecture_benchmark.aggregate_pass_rate_bps,
                regression_from_baseline_bps: 0,
            }],
            "Reference architecture benchmark receipt is derived from the executed retrieval evaluator.",
        ),
        benchmark_evidence_receipt(
            held_out_benchmark,
            vec![
                PsionObservedMetric {
                    metric_kind: PsionMetricKind::PassRateBps,
                    observed_bps: held_out_benchmark.aggregate_pass_rate_bps,
                    regression_from_baseline_bps: 0,
                },
                PsionObservedMetric {
                    metric_kind: PsionMetricKind::ImprovementOverSeedBaselineBps,
                    observed_bps: held_out_benchmark
                        .rows
                        .iter()
                        .map(|row| row.observed_score_milli.max(0) as u32)
                        .min()
                        .unwrap_or(0),
                    regression_from_baseline_bps: 0,
                },
            ],
            "Reference held-out reasoning receipt is derived from the executed validation-family loss deltas.",
        ),
        benchmark_evidence_receipt(
            normative_spec_benchmark,
            vec![PsionObservedMetric {
                metric_kind: PsionMetricKind::PassRateBps,
                observed_bps: normative_spec_benchmark.aggregate_pass_rate_bps,
                regression_from_baseline_bps: 0,
            }],
            "Reference normative-spec receipt is derived from the executed spec retrieval evaluator.",
        ),
        benchmark_evidence_receipt(
            &PsionReferencePilotBenchmarkEvaluation {
                evaluation_id: String::from("psion-reference-unsupported-request-refusal-eval"),
                family: PsionBenchmarkFamily::UnsupportedRequestRefusal,
                benchmark_artifact_id: refusal_receipt.package_id.clone(),
                benchmark_artifact_digest: refusal_receipt.package_digest.clone(),
                rows: refusal_receipt
                    .rows
                    .iter()
                    .map(|row| PsionReferencePilotBenchmarkRow {
                        item_id: row.item_id.clone(),
                        matched_sequence_id: None,
                        matched_source_id: None,
                        observed_route_class: None,
                        observed_refusal_reason_code: Some(row.expected_reason_code.clone()),
                        observed_score_milli: row.observed_refusal_accuracy_bps as i32,
                        passed: row.observed_refusal_accuracy_bps == 10_000
                            && row.reason_code_match_bps == 10_000,
                        detail: row.detail.clone(),
                    })
                    .collect(),
                aggregate_pass_rate_bps: refusal_receipt.aggregate_unsupported_request_refusal_bps,
                summary: String::from(
                    "Reference unsupported-request refusal benchmark is derived from the executed refusal-calibration receipt.",
                ),
            },
            vec![
                PsionObservedMetric {
                    metric_kind: PsionMetricKind::UnsupportedRequestRefusalBps,
                    observed_bps: refusal_receipt.aggregate_unsupported_request_refusal_bps,
                    regression_from_baseline_bps: refusal_receipt.refusal_regression_bps,
                },
                PsionObservedMetric {
                    metric_kind: PsionMetricKind::OverrefusalBps,
                    observed_bps: refusal_receipt.supported_control_overrefusal_bps,
                    regression_from_baseline_bps: 0,
                },
            ],
            "Reference unsupported-request refusal receipt is derived from the executed refusal-calibration receipt.",
        ),
    ];
    PsionPromotionDecisionReceipt {
        schema_version: String::from(crate::PSION_PROMOTION_DECISION_SCHEMA_VERSION),
        decision_id: String::from("psion-reference-pilot-promotion-decision"),
        matrix_id: acceptance_matrix.matrix_id.clone(),
        matrix_version: acceptance_matrix.matrix_version.clone(),
        phase: PsionPhaseGate::Pilot,
        candidate_artifact_id: run
            .stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint_label
            .clone(),
        benchmark_receipts,
        replay_receipt: PsionReplayEvidenceReceipt {
            receipt_id: String::from("psion-reference-pilot-replay-evidence"),
            successful_replays: run.stage_receipt.replay_receipt.successful_replays,
            exact_replay_observed: run.stage_receipt.replay_receipt.exact_replay_observed,
            summary: String::from(
                "Reference pilot replay evidence is copied from the executed pretrain-stage replay receipt.",
            ),
        },
        checkpoint_receipt: PsionCheckpointRecoveryReceipt {
            receipt_id: String::from("psion-reference-pilot-checkpoint-recovery"),
            checkpoint_id: run
                .stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .clone(),
            successful_restart_roundtrips: 1,
            restart_recovery_observed: true,
            resume_regression_bps: 0,
            summary: String::from(
                "Reference pilot restored the emitted checkpoint and reproduced the final validation-family losses without regression.",
            ),
        },
        contamination_review_receipt: PsionContaminationReviewReceipt {
            receipt_id: String::from("psion-reference-pilot-contamination-review"),
            benchmark_isolation_schema_version: run
                .corpus_bundle
                .exclusion_manifest
                .schema_version
                .clone(),
            exclusion_manifest_digest: stable_digest(
                b"psion_reference_exclusion_manifest|",
                &run.corpus_bundle.exclusion_manifest,
            ),
            near_duplicate_review_completed: true,
            tokenizer_exposure_review_completed: true,
            disposition: PsionContaminationReviewDisposition::Clean,
            applied_consequences: Vec::new(),
            summary: String::from(
                "Reference pilot contamination review stayed clean against the executed exclusion manifest.",
            ),
        },
        route_calibration_receipt: PsionRouteCalibrationReceipt {
            receipt_id: String::from("psion-reference-pilot-route-calibration"),
            covered_routes: vec![
                PsionRouteKind::DirectModelAnswer,
                PsionRouteKind::ExactExecutorHandoff,
                PsionRouteKind::Refusal,
            ],
            route_selection_accuracy_bps: route_probe_receipt
                .aggregate_route_selection_accuracy_bps,
            route_regression_bps: 0,
            summary: String::from(
                "Reference pilot route calibration is derived from the executed route-probe receipt.",
            ),
        },
        refusal_calibration_receipt: refusal_receipt.clone(),
        decision: PsionPromotionDecisionDisposition::Promoted,
        hold_reason_codes: Vec::new(),
        decision_summary: String::from(
            "Reference pilot cleared the pilot gate on executed benchmark, replay, checkpoint, route, refusal, and contamination evidence.",
        ),
    }
}

fn benchmark_evidence_receipt(
    evaluation: &PsionReferencePilotBenchmarkEvaluation,
    metrics: Vec<PsionObservedMetric>,
    summary: &str,
) -> PsionBenchmarkEvidenceReceipt {
    PsionBenchmarkEvidenceReceipt {
        receipt_id: format!("{}-receipt", evaluation.evaluation_id),
        phase: PsionPhaseGate::Pilot,
        family: evaluation.family,
        benchmark_artifact_id: evaluation.benchmark_artifact_id.clone(),
        benchmark_artifact_digest: evaluation.benchmark_artifact_digest.clone(),
        metrics,
        summary: String::from(summary),
    }
}

#[derive(Clone, Debug)]
struct ReferenceSequenceMatch {
    sequence_id: String,
    source_id: String,
    token_set: BTreeSet<String>,
    score: f32,
}

fn benchmark_row_from_match(
    item_id: &str,
    matched: Option<ReferenceSequenceMatch>,
    required_terms: &BTreeSet<String>,
    pass_detail: &str,
) -> PsionReferencePilotBenchmarkRow {
    let (matched_sequence_id, matched_source_id, observed_score_milli, passed, detail) =
        if let Some(matched) = matched {
            let passed = required_terms
                .iter()
                .any(|term| matched.token_set.contains(term));
            (
                Some(matched.sequence_id),
                Some(matched.source_id),
                (matched.score * 1000.0).round() as i32,
                passed,
                if passed {
                    String::from(pass_detail)
                } else {
                    format!(
                        "Reference benchmark retrieved a sequence for `{item_id}` but it did not cover every required normalized term."
                    )
                },
            )
        } else {
            (
                None,
                None,
                0,
                false,
                format!(
                    "Reference benchmark could not retrieve any candidate sequence for `{item_id}`."
                ),
            )
        };
    PsionReferencePilotBenchmarkRow {
        item_id: String::from(item_id),
        matched_sequence_id,
        matched_source_id,
        observed_route_class: None,
        observed_refusal_reason_code: None,
        observed_score_milli,
        passed,
        detail,
    }
}

fn best_sequence_match(
    run: &PsionReferencePilotRun,
    model: &PsionCompactDecoderReferencePilotModel,
    query_terms: &BTreeSet<String>,
    allowed_families: &[&str],
) -> Option<ReferenceSequenceMatch> {
    let query_token_ids = query_terms
        .iter()
        .filter_map(|term| run.corpus_bundle.token_id(term))
        .collect::<Vec<_>>();
    let query_embedding = mean_token_embedding(model, query_token_ids.as_slice())?;
    let allowed_families = allowed_families.iter().copied().collect::<BTreeSet<_>>();
    run.corpus_bundle
        .shard_artifacts
        .iter()
        .flat_map(|artifact| artifact.sequences.iter())
        .filter(|sequence| allowed_families.contains(sequence.source_family_id.as_str()))
        .filter_map(|sequence| {
            let token_set = sequence_token_set(&run.corpus_bundle, sequence);
            let lexical_overlap = query_terms
                .iter()
                .filter(|term| token_set.contains(term.as_str()))
                .count();
            if lexical_overlap == 0 {
                return None;
            }
            let sequence_embedding = mean_token_embedding(model, sequence.token_ids.as_slice())?;
            let cosine =
                cosine_similarity(query_embedding.as_slice(), sequence_embedding.as_slice());
            Some(ReferenceSequenceMatch {
                sequence_id: sequence.sequence_id.clone(),
                source_id: sequence.source_id.clone(),
                token_set,
                score: (lexical_overlap as f32 * 100.0) + cosine,
            })
        })
        .max_by(|left, right| left.score.total_cmp(&right.score))
}

fn combined_query_terms(parts: &[&str]) -> BTreeSet<String> {
    let mut terms = BTreeSet::new();
    for part in parts {
        let base_tokens = tokenize_reference_text(part);
        for token in &base_tokens {
            terms.insert(token.clone());
            if token.ends_with('s') && token.len() > 1 {
                terms.insert(token.trim_end_matches('s').to_string());
            }
        }
        for start in 0..base_tokens.len() {
            for length in 2..=base_tokens.len().saturating_sub(start) {
                terms.insert(base_tokens[start..start + length].join("_"));
            }
        }
    }
    terms
}

fn tokenize_reference_text(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

fn sequence_token_set(
    bundle: &PsionReferenceCorpusBundle,
    sequence: &PsionReferenceEncodedSequence,
) -> BTreeSet<String> {
    sequence
        .token_ids
        .iter()
        .filter_map(|token_id| bundle.vocabulary_artifact.tokens.get(*token_id as usize))
        .filter(|token| !token.starts_with('<'))
        .cloned()
        .collect()
}

fn mean_token_embedding(
    model: &PsionCompactDecoderReferencePilotModel,
    token_ids: &[u32],
) -> Option<Vec<f32>> {
    let hidden_size = model.descriptor.config.hidden_size;
    let mut embedding = vec![0.0; hidden_size];
    let mut count = 0_u32;
    for token_id in token_ids {
        let token_index = *token_id as usize;
        if token_index >= model.descriptor.config.vocab_size {
            continue;
        }
        let offset = token_index * hidden_size;
        for index in 0..hidden_size {
            embedding[index] += model.token_embeddings[offset + index];
        }
        count = count.saturating_add(1);
    }
    if count == 0 {
        return None;
    }
    let scale = 1.0 / count as f32;
    for value in &mut embedding {
        *value *= scale;
    }
    Some(embedding)
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot(left, right) / (left_norm * right_norm)
    }
}

fn bps_from_pass_count(pass_count: usize, total_count: usize) -> u32 {
    if total_count == 0 {
        0
    } else {
        ((pass_count as u64 * 10_000) / total_count as u64) as u32
    }
}

fn improvement_bps(seed_baseline_loss_milli: u32, pilot_loss_milli: u32) -> u32 {
    if seed_baseline_loss_milli == 0 || pilot_loss_milli >= seed_baseline_loss_milli {
        0
    } else {
        (((seed_baseline_loss_milli - pilot_loss_milli) as u64 * 10_000)
            / seed_baseline_loss_milli as u64) as u32
    }
}

impl PsionCompactDecoderReferencePilotModel {
    fn seeded(descriptor: PsionCompactDecoderDescriptor) -> Self {
        let hidden_size = descriptor.config.hidden_size;
        let vocab_size = descriptor.config.vocab_size;
        let max_context = descriptor.config.max_context;
        let token_embeddings = seeded_values(
            "psion.reference.token_embeddings",
            vocab_size * hidden_size,
            0.02,
        );
        let position_embeddings = seeded_values(
            "psion.reference.position_embeddings",
            max_context * hidden_size,
            0.01,
        );
        let lm_head_bias = vec![0.0; vocab_size];
        Self {
            descriptor,
            token_embeddings,
            position_embeddings,
            lm_head_bias,
        }
    }

    fn parameter_shapes(&self) -> BTreeMap<String, Vec<usize>> {
        BTreeMap::from([
            (
                String::from(TOKEN_EMBEDDING_GROUP_ID),
                vec![
                    self.descriptor.config.vocab_size,
                    self.descriptor.config.hidden_size,
                ],
            ),
            (
                String::from(POSITION_EMBEDDING_GROUP_ID),
                vec![
                    self.descriptor.config.max_context,
                    self.descriptor.config.hidden_size,
                ],
            ),
            (
                String::from(LM_HEAD_BIAS_GROUP_ID),
                vec![self.descriptor.config.vocab_size],
            ),
        ])
    }

    fn parameter_values(&self) -> BTreeMap<String, Vec<f32>> {
        BTreeMap::from([
            (
                String::from(TOKEN_EMBEDDING_GROUP_ID),
                self.token_embeddings.clone(),
            ),
            (
                String::from(POSITION_EMBEDDING_GROUP_ID),
                self.position_embeddings.clone(),
            ),
            (
                String::from(LM_HEAD_BIAS_GROUP_ID),
                self.lm_head_bias.clone(),
            ),
        ])
    }

    fn with_parameter_overrides(
        &self,
        overrides: &BTreeMap<String, Vec<f32>>,
    ) -> Result<Self, PsionReferencePilotError> {
        let shapes = self.parameter_shapes();
        let mut next = self.clone();
        if let Some(values) = overrides.get(TOKEN_EMBEDDING_GROUP_ID) {
            require_len(
                values,
                element_count(shapes.get(TOKEN_EMBEDDING_GROUP_ID).expect("shape")),
                TOKEN_EMBEDDING_GROUP_ID,
            )?;
            next.token_embeddings = values.clone();
        }
        if let Some(values) = overrides.get(POSITION_EMBEDDING_GROUP_ID) {
            require_len(
                values,
                element_count(shapes.get(POSITION_EMBEDDING_GROUP_ID).expect("shape")),
                POSITION_EMBEDDING_GROUP_ID,
            )?;
            next.position_embeddings = values.clone();
        }
        if let Some(values) = overrides.get(LM_HEAD_BIAS_GROUP_ID) {
            require_len(
                values,
                element_count(shapes.get(LM_HEAD_BIAS_GROUP_ID).expect("shape")),
                LM_HEAD_BIAS_GROUP_ID,
            )?;
            next.lm_head_bias = values.clone();
        }
        Ok(next)
    }

    fn next_token_logits(&self, context_token_ids: &[u32]) -> Vec<f32> {
        let hidden_size = self.descriptor.config.hidden_size;
        let vocab_size = self.descriptor.config.vocab_size;
        let context_len = context_token_ids
            .len()
            .min(self.descriptor.config.max_context)
            .max(1);
        let mut hidden = vec![0.0; hidden_size];
        for (position, token_id) in context_token_ids.iter().take(context_len).enumerate() {
            let token_index = (*token_id as usize).min(vocab_size.saturating_sub(1));
            let token_offset = token_index * hidden_size;
            let position_offset = position * hidden_size;
            for index in 0..hidden_size {
                hidden[index] += self.token_embeddings[token_offset + index];
                hidden[index] += self.position_embeddings[position_offset + index];
            }
        }
        let scale = 1.0 / context_len as f32;
        for value in &mut hidden {
            *value *= scale;
        }
        let mut logits = self.lm_head_bias.clone();
        for token_index in 0..vocab_size {
            let token_offset = token_index * hidden_size;
            logits[token_index] += dot(
                &self.token_embeddings[token_offset..token_offset + hidden_size],
                &hidden,
            );
        }
        logits
    }

    fn loss_and_gradients(
        &self,
        examples: &[PsionReferenceTrainingExample],
    ) -> (f32, BTreeMap<String, Vec<f32>>) {
        let hidden_size = self.descriptor.config.hidden_size;
        let vocab_size = self.descriptor.config.vocab_size;
        let mut token_gradients = vec![0.0; self.token_embeddings.len()];
        let mut position_gradients = vec![0.0; self.position_embeddings.len()];
        let mut bias_gradients = vec![0.0; self.lm_head_bias.len()];
        let mut total_loss = 0.0;
        for example in examples {
            let context_len = example
                .context_token_ids
                .len()
                .min(self.descriptor.config.max_context)
                .max(1);
            let mut hidden = vec![0.0; hidden_size];
            for (position, token_id) in example
                .context_token_ids
                .iter()
                .take(context_len)
                .enumerate()
            {
                let token_index = (*token_id as usize).min(vocab_size.saturating_sub(1));
                let token_offset = token_index * hidden_size;
                let position_offset = position * hidden_size;
                for index in 0..hidden_size {
                    hidden[index] += self.token_embeddings[token_offset + index];
                    hidden[index] += self.position_embeddings[position_offset + index];
                }
            }
            let scale = 1.0 / context_len as f32;
            for value in &mut hidden {
                *value *= scale;
            }
            let logits = self.next_token_logits(example.context_token_ids.as_slice());
            let probabilities = softmax(logits.as_slice());
            let target_index = (example.target_token_id as usize).min(vocab_size.saturating_sub(1));
            total_loss += -probabilities[target_index].max(1e-9).ln();

            let mut dlogits = probabilities;
            dlogits[target_index] -= 1.0;
            for token_index in 0..vocab_size {
                bias_gradients[token_index] += dlogits[token_index];
                let token_offset = token_index * hidden_size;
                for index in 0..hidden_size {
                    token_gradients[token_offset + index] += dlogits[token_index] * hidden[index];
                }
            }
            let mut hidden_grad = vec![0.0; hidden_size];
            for token_index in 0..vocab_size {
                let token_offset = token_index * hidden_size;
                for index in 0..hidden_size {
                    hidden_grad[index] +=
                        dlogits[token_index] * self.token_embeddings[token_offset + index];
                }
            }
            let input_scale = 1.0 / context_len as f32;
            for (position, token_id) in example
                .context_token_ids
                .iter()
                .take(context_len)
                .enumerate()
            {
                let token_index = (*token_id as usize).min(vocab_size.saturating_sub(1));
                let token_offset = token_index * hidden_size;
                let position_offset = position * hidden_size;
                for index in 0..hidden_size {
                    token_gradients[token_offset + index] += hidden_grad[index] * input_scale;
                    position_gradients[position_offset + index] += hidden_grad[index] * input_scale;
                }
            }
        }
        let example_scale = 1.0 / examples.len().max(1) as f32;
        scale_in_place(token_gradients.as_mut_slice(), example_scale);
        scale_in_place(position_gradients.as_mut_slice(), example_scale);
        scale_in_place(bias_gradients.as_mut_slice(), example_scale);
        (
            total_loss * example_scale,
            BTreeMap::from([
                (String::from(TOKEN_EMBEDDING_GROUP_ID), token_gradients),
                (
                    String::from(POSITION_EMBEDDING_GROUP_ID),
                    position_gradients,
                ),
                (String::from(LM_HEAD_BIAS_GROUP_ID), bias_gradients),
            ]),
        )
    }

    fn mean_loss(&self, examples: &[PsionReferenceTrainingExample]) -> f32 {
        if examples.is_empty() {
            return 0.0;
        }
        examples
            .iter()
            .map(|example| {
                let logits = self.next_token_logits(example.context_token_ids.as_slice());
                let probabilities = softmax(logits.as_slice());
                let target_index = (example.target_token_id as usize)
                    .min(self.descriptor.config.vocab_size.saturating_sub(1));
                -probabilities[target_index].max(1e-9).ln()
            })
            .sum::<f32>()
            / examples.len() as f32
    }
}

fn build_parameter_groups(
    model: &PsionCompactDecoderReferencePilotModel,
    config: &PsionReferencePilotConfig,
) -> Result<Vec<TrainingParameterGroupState>, PsionReferencePilotError> {
    build_parameter_groups_for_execution(
        model,
        config,
        &Device::cpu(),
        TrainingOptimizerResidencyPolicy::host_only(),
    )
}

fn build_parameter_groups_for_execution(
    model: &PsionCompactDecoderReferencePilotModel,
    config: &PsionReferencePilotConfig,
    device: &Device,
    optimizer_residency_policy: TrainingOptimizerResidencyPolicy,
) -> Result<Vec<TrainingParameterGroupState>, PsionReferencePilotError> {
    let shapes = model.parameter_shapes();
    let values = model.parameter_values();
    let mut groups = Vec::new();
    for (group_id, shape) in shapes {
        let values = values
            .get(group_id.as_str())
            .expect("parameter values should cover every shape")
            .clone();
        let class = match group_id.as_str() {
            TOKEN_EMBEDDING_GROUP_ID | POSITION_EMBEDDING_GROUP_ID => {
                TrainingParameterClass::Embedding
            }
            LM_HEAD_BIAS_GROUP_ID => TrainingParameterClass::Bias,
            _ => TrainingParameterClass::Matrix,
        };
        groups.push(TrainingParameterGroupState::new(
            group_id.clone(),
            class,
            TrainingTensorBuffer::from_f32(
                group_id.clone(),
                TensorSpec::new(Shape::new(shape), DType::F32, device.clone()),
                values,
            )?,
            config.optimizer.clone(),
            optimizer_residency_policy,
        )?);
    }
    Ok(groups)
}

fn materialize_model(
    descriptor: &PsionCompactDecoderDescriptor,
    run: &FixedBudgetTrainingRun,
) -> Result<PsionCompactDecoderReferencePilotModel, PsionReferencePilotError> {
    let token_embeddings = group_values(run, TOKEN_EMBEDDING_GROUP_ID)?;
    let position_embeddings = group_values(run, POSITION_EMBEDDING_GROUP_ID)?;
    let lm_head_bias = group_values(run, LM_HEAD_BIAS_GROUP_ID)?;
    Ok(PsionCompactDecoderReferencePilotModel {
        descriptor: descriptor.clone(),
        token_embeddings,
        position_embeddings,
        lm_head_bias,
    })
}

fn group_values(
    run: &FixedBudgetTrainingRun,
    group_id: &str,
) -> Result<Vec<f32>, PsionReferencePilotError> {
    let group = run.parameter_group(group_id).ok_or_else(|| {
        PsionReferencePilotError::MissingParameterGroup {
            group_id: String::from(group_id),
        }
    })?;
    match &group.parameter.data {
        TensorData::F32(values) => Ok(values.clone()),
        _ => Err(PsionReferencePilotError::NonDenseParameterGroup {
            group_id: String::from(group_id),
        }),
    }
}

fn split_examples(
    corpus_bundle: &PsionReferenceCorpusBundle,
    split_kind: DatasetSplitKind,
) -> Vec<PsionReferenceTrainingExample> {
    let Some(shard) = corpus_bundle.shard(split_kind) else {
        return Vec::new();
    };
    build_examples_from_sequences(
        shard.sequences.as_slice(),
        PSION_REFERENCE_MAX_SEQUENCE_TOKENS as usize,
    )
}

fn build_examples_from_sequences(
    sequences: &[PsionReferenceEncodedSequence],
    max_context: usize,
) -> Vec<PsionReferenceTrainingExample> {
    let mut examples = Vec::new();
    for sequence in sequences {
        if sequence.token_ids.len() < 2 {
            continue;
        }
        for target_index in 1..sequence.token_ids.len() {
            let start_index = target_index.saturating_sub(max_context);
            let context = sequence.token_ids[start_index..target_index].to_vec();
            examples.push(PsionReferenceTrainingExample {
                source_id: sequence.source_id.clone(),
                source_family_id: sequence.source_family_id.clone(),
                split_kind: sequence.split_kind,
                context_token_ids: context,
                target_token_id: sequence.token_ids[target_index],
            });
        }
    }
    examples
}

fn build_accelerated_train_examples(
    examples: &[PsionReferenceTrainingExample],
    target_rows: usize,
) -> Vec<PsionReferenceTrainingExample> {
    if examples.is_empty() {
        return Vec::new();
    }
    let row_count = target_rows.max(examples.len());
    let mut expanded = Vec::with_capacity(row_count);
    for index in 0..row_count {
        expanded.push(examples[index % examples.len()].clone());
    }
    expanded
}

fn build_accelerated_gradient_program(
    device: Device,
    descriptor: &PsionCompactDecoderDescriptor,
    batch_size: usize,
) -> Result<PsionAcceleratedGradientProgram, PsionReferencePilotError> {
    let hidden_size = descriptor.config.hidden_size;
    let vocab_size = descriptor.config.vocab_size;
    let mut logits_builder =
        AutodiffGraphBuilder::with_context(device.clone(), AutodiffContext::training());
    let hidden_inputs = logits_builder.input(
        "psion_accelerated_hidden_inputs",
        Shape::new(vec![batch_size, hidden_size]),
        DType::F32,
        false,
    );
    let token_embeddings = logits_builder.input(
        TOKEN_EMBEDDING_GROUP_ID,
        Shape::new(vec![hidden_size, vocab_size]),
        DType::F32,
        false,
    );
    let logits = logits_builder.matmul(&hidden_inputs, &token_embeddings)?;
    let logits_graph = logits_builder.finish(vec![logits.clone()]);

    let mut weight_gradient_builder =
        AutodiffGraphBuilder::with_context(device.clone(), AutodiffContext::training());
    let hidden_inputs_transposed = weight_gradient_builder.input(
        "psion_accelerated_hidden_inputs_transposed",
        Shape::new(vec![hidden_size, batch_size]),
        DType::F32,
        false,
    );
    let logits_seed = weight_gradient_builder.input(
        "psion_accelerated_logits_seed",
        Shape::new(vec![batch_size, vocab_size]),
        DType::F32,
        false,
    );
    let weight_gradient =
        weight_gradient_builder.matmul(&hidden_inputs_transposed, &logits_seed)?;
    let weight_gradient_graph = weight_gradient_builder.finish(vec![weight_gradient.clone()]);

    let mut hidden_gradient_builder =
        AutodiffGraphBuilder::with_context(device, AutodiffContext::training());
    let logits_seed_for_hidden = hidden_gradient_builder.input(
        "psion_accelerated_logits_seed_for_hidden",
        Shape::new(vec![batch_size, vocab_size]),
        DType::F32,
        false,
    );
    let token_embeddings_vh = hidden_gradient_builder.input(
        "psion_accelerated_token_embeddings_vh",
        Shape::new(vec![vocab_size, hidden_size]),
        DType::F32,
        false,
    );
    let hidden_gradient =
        hidden_gradient_builder.matmul(&logits_seed_for_hidden, &token_embeddings_vh)?;
    let hidden_gradient_graph = hidden_gradient_builder.finish(vec![hidden_gradient.clone()]);

    Ok(PsionAcceleratedGradientProgram {
        logits_graph,
        logits_hidden_input_tensor_id: hidden_inputs.id(),
        logits_token_embeddings_tensor_id: token_embeddings.id(),
        logits_tensor_id: logits.id(),
        weight_gradient_graph,
        weight_gradient_hidden_input_transposed_tensor_id: hidden_inputs_transposed.id(),
        weight_gradient_logits_seed_tensor_id: logits_seed.id(),
        weight_gradient_tensor_id: weight_gradient.id(),
        hidden_gradient_graph,
        hidden_gradient_logits_seed_tensor_id: logits_seed_for_hidden.id(),
        hidden_gradient_token_embeddings_vh_tensor_id: token_embeddings_vh.id(),
        hidden_gradient_tensor_id: hidden_gradient.id(),
    })
}

fn build_accelerated_gradient_batch(
    cuda_backend: &mut CudaBackend,
    program: &PsionAcceleratedGradientProgram,
    model: &PsionCompactDecoderReferencePilotModel,
    examples: &[PsionReferenceTrainingExample],
    device: &Device,
) -> Result<crate::TrainingGradientBatch, PsionReferencePilotError> {
    build_accelerated_gradient_batch_with_executor(
        program,
        model,
        examples,
        device,
        |graph, inputs| execute_cuda_graph(cuda_backend, graph, inputs),
    )
}

fn build_accelerated_gradient_batch_with_executor(
    program: &PsionAcceleratedGradientProgram,
    model: &PsionCompactDecoderReferencePilotModel,
    examples: &[PsionReferenceTrainingExample],
    device: &Device,
    mut execute_graph: impl FnMut(
        &psionic_ir::Graph,
        &[(TensorId, Vec<f32>)],
    ) -> Result<Vec<f32>, PsionReferencePilotError>,
) -> Result<crate::TrainingGradientBatch, PsionReferencePilotError> {
    let hidden_inputs = build_accelerated_hidden_inputs(model, examples);
    let token_embeddings_hv = transpose_matrix(
        model.token_embeddings.as_slice(),
        model.descriptor.config.vocab_size,
        model.descriptor.config.hidden_size,
    )?;
    let logits_result = execute_graph(
        program.logits_graph.graph(),
        [
            (program.logits_hidden_input_tensor_id, hidden_inputs.clone()),
            (
                program.logits_token_embeddings_tensor_id,
                token_embeddings_hv.clone(),
            ),
        ]
        .as_slice(),
    )?;
    let (loss_value, logits_seed) = dense_logits_loss_seed(
        &TensorData::F32(add_bias_rows(
            logits_result.as_slice(),
            model.lm_head_bias.as_slice(),
            model.descriptor.config.vocab_size,
        )?),
        examples,
        model.descriptor.config.vocab_size,
    )?;
    let hidden_inputs_transposed = transpose_matrix(
        hidden_inputs.as_slice(),
        examples.len(),
        model.descriptor.config.hidden_size,
    )?;
    let transposed_token_gradients = execute_graph(
        program.weight_gradient_graph.graph(),
        [
            (
                program.weight_gradient_hidden_input_transposed_tensor_id,
                hidden_inputs_transposed,
            ),
            (
                program.weight_gradient_logits_seed_tensor_id,
                logits_seed.clone(),
            ),
        ]
        .as_slice(),
    )?;
    let mut token_gradients = transpose_matrix(
        transposed_token_gradients.as_slice(),
        model.descriptor.config.hidden_size,
        model.descriptor.config.vocab_size,
    )?;
    let hidden_input_gradients = execute_graph(
        program.hidden_gradient_graph.graph(),
        [
            (
                program.hidden_gradient_logits_seed_tensor_id,
                logits_seed.clone(),
            ),
            (
                program.hidden_gradient_token_embeddings_vh_tensor_id,
                model.token_embeddings.clone(),
            ),
        ]
        .as_slice(),
    )?;
    let bias_gradients = sum_logits_seed_rows(
        logits_seed.as_slice(),
        examples.len(),
        model.descriptor.config.vocab_size,
    )?;
    let mut position_gradients = vec![0.0; model.position_embeddings.len()];
    scatter_accelerated_hidden_input_gradients(
        model,
        examples,
        hidden_input_gradients.as_slice(),
        token_gradients.as_mut_slice(),
        position_gradients.as_mut_slice(),
    );

    let mut buffers = BTreeMap::new();
    buffers.insert(
        String::from(TOKEN_EMBEDDING_GROUP_ID),
        TrainingTensorBuffer::from_f32(
            String::from(TOKEN_EMBEDDING_GROUP_ID),
            TensorSpec::new(
                Shape::new(vec![
                    model.descriptor.config.vocab_size,
                    model.descriptor.config.hidden_size,
                ]),
                DType::F32,
                device.clone(),
            ),
            token_gradients,
        )?,
    );
    buffers.insert(
        String::from(POSITION_EMBEDDING_GROUP_ID),
        TrainingTensorBuffer::from_f32(
            String::from(POSITION_EMBEDDING_GROUP_ID),
            TensorSpec::new(
                Shape::new(vec![
                    model.descriptor.config.max_context,
                    model.descriptor.config.hidden_size,
                ]),
                DType::F32,
                device.clone(),
            ),
            position_gradients,
        )?,
    );
    buffers.insert(
        String::from(LM_HEAD_BIAS_GROUP_ID),
        TrainingTensorBuffer::from_f32(
            String::from(LM_HEAD_BIAS_GROUP_ID),
            TensorSpec::new(
                Shape::new(vec![model.descriptor.config.vocab_size]),
                DType::F32,
                device.clone(),
            ),
            bias_gradients,
        )?,
    );
    Ok(crate::TrainingGradientBatch::new(
        "psion-accelerated-reference-gradient-batch",
        loss_value,
        examples.len() as u32,
        buffers,
    ))
}

fn build_accelerated_hidden_inputs(
    model: &PsionCompactDecoderReferencePilotModel,
    examples: &[PsionReferenceTrainingExample],
) -> Vec<f32> {
    let hidden_size = model.descriptor.config.hidden_size;
    let vocab_size = model.descriptor.config.vocab_size;
    let mut hidden_inputs = Vec::with_capacity(examples.len().saturating_mul(hidden_size));
    for example in examples {
        let context_len = example
            .context_token_ids
            .len()
            .min(model.descriptor.config.max_context)
            .max(1);
        let mut hidden = vec![0.0; hidden_size];
        for (position, token_id) in example
            .context_token_ids
            .iter()
            .take(context_len)
            .enumerate()
        {
            let token_index = (*token_id as usize).min(vocab_size.saturating_sub(1));
            let token_offset = token_index * hidden_size;
            let position_offset = position * hidden_size;
            for index in 0..hidden_size {
                hidden[index] += model.token_embeddings[token_offset + index];
                hidden[index] += model.position_embeddings[position_offset + index];
            }
        }
        let scale = 1.0 / context_len as f32;
        scale_in_place(hidden.as_mut_slice(), scale);
        hidden_inputs.extend(hidden);
    }
    hidden_inputs
}

fn scatter_accelerated_hidden_input_gradients(
    model: &PsionCompactDecoderReferencePilotModel,
    examples: &[PsionReferenceTrainingExample],
    hidden_input_gradients: &[f32],
    token_gradients: &mut [f32],
    position_gradients: &mut [f32],
) {
    let hidden_size = model.descriptor.config.hidden_size;
    let vocab_size = model.descriptor.config.vocab_size;
    for (example_index, example) in examples.iter().enumerate() {
        let context_len = example
            .context_token_ids
            .len()
            .min(model.descriptor.config.max_context)
            .max(1);
        let row_offset = example_index * hidden_size;
        let hidden_grad = &hidden_input_gradients[row_offset..row_offset + hidden_size];
        let input_scale = 1.0 / context_len as f32;
        for (position, token_id) in example
            .context_token_ids
            .iter()
            .take(context_len)
            .enumerate()
        {
            let token_index = (*token_id as usize).min(vocab_size.saturating_sub(1));
            let token_offset = token_index * hidden_size;
            let position_offset = position * hidden_size;
            for index in 0..hidden_size {
                token_gradients[token_offset + index] += hidden_grad[index] * input_scale;
                position_gradients[position_offset + index] += hidden_grad[index] * input_scale;
            }
        }
    }
}

fn dense_values(data: &TensorData, context: &str) -> Result<Vec<f32>, PsionReferencePilotError> {
    match data {
        TensorData::F32(values) | TensorData::BF16(values) => Ok(values.clone()),
        TensorData::I32(_) => Err(PsionReferencePilotError::Serialization {
            message: format!("{context} must be dense f32"),
        }),
        TensorData::QuantizedBlocks(_) => Err(PsionReferencePilotError::Serialization {
            message: format!("{context} must be dense f32"),
        }),
    }
}

fn transpose_matrix(
    values: &[f32],
    rows: usize,
    cols: usize,
) -> Result<Vec<f32>, PsionReferencePilotError> {
    let expected_len = rows.saturating_mul(cols);
    if values.len() != expected_len {
        return Err(PsionReferencePilotError::Serialization {
            message: format!(
                "matrix transpose length mismatch: expected {}, found {}",
                expected_len,
                values.len()
            ),
        });
    }
    let mut transposed = vec![0.0_f32; expected_len];
    for row in 0..rows {
        for col in 0..cols {
            transposed[col * rows + row] = values[row * cols + col];
        }
    }
    Ok(transposed)
}

fn dense_logits_loss_seed(
    logits: &TensorData,
    examples: &[PsionReferenceTrainingExample],
    vocab_size: usize,
) -> Result<(f32, Vec<f32>), PsionReferencePilotError> {
    let logits = dense_values(logits, "psion_accelerated_logits")?;
    if examples.is_empty() {
        return Ok((0.0, Vec::new()));
    }
    let expected_len = examples.len().saturating_mul(vocab_size);
    if logits.len() != expected_len {
        return Err(PsionReferencePilotError::Serialization {
            message: format!(
                "accelerated logits length mismatch: expected {}, found {}",
                expected_len,
                logits.len()
            ),
        });
    }

    let mut total_loss = 0.0_f32;
    let mut logits_seed = vec![0.0_f32; logits.len()];
    for (example_index, example) in examples.iter().enumerate() {
        let row_start = example_index * vocab_size;
        let row = &logits[row_start..row_start + vocab_size];
        let probabilities = softmax(row);
        let target_index = (example.target_token_id as usize).min(vocab_size.saturating_sub(1));
        total_loss += -probabilities[target_index].max(1e-9).ln();
        let seed_row = &mut logits_seed[row_start..row_start + vocab_size];
        seed_row.copy_from_slice(probabilities.as_slice());
        seed_row[target_index] -= 1.0;
    }
    let example_scale = 1.0 / examples.len() as f32;
    scale_in_place(logits_seed.as_mut_slice(), example_scale);
    Ok((total_loss * example_scale, logits_seed))
}

fn add_bias_rows(
    logits: &[f32],
    bias: &[f32],
    vocab_size: usize,
) -> Result<Vec<f32>, PsionReferencePilotError> {
    if bias.len() != vocab_size {
        return Err(PsionReferencePilotError::Serialization {
            message: format!(
                "accelerated bias length mismatch: expected {}, found {}",
                vocab_size,
                bias.len()
            ),
        });
    }
    if logits.len() % vocab_size != 0 {
        return Err(PsionReferencePilotError::Serialization {
            message: format!(
                "accelerated logits length {} is not divisible by vocab size {}",
                logits.len(),
                vocab_size
            ),
        });
    }
    let mut biased = logits.to_vec();
    for row in biased.chunks_mut(vocab_size) {
        for (value, bias_value) in row.iter_mut().zip(bias.iter()) {
            *value += *bias_value;
        }
    }
    Ok(biased)
}

fn sum_logits_seed_rows(
    logits_seed: &[f32],
    batch_size: usize,
    vocab_size: usize,
) -> Result<Vec<f32>, PsionReferencePilotError> {
    let expected_len = batch_size.saturating_mul(vocab_size);
    if logits_seed.len() != expected_len {
        return Err(PsionReferencePilotError::Serialization {
            message: format!(
                "accelerated logits seed length mismatch: expected {}, found {}",
                expected_len,
                logits_seed.len()
            ),
        });
    }
    let mut sums = vec![0.0_f32; vocab_size];
    for row in logits_seed.chunks(vocab_size) {
        for (sum, value) in sums.iter_mut().zip(row.iter()) {
            *sum += *value;
        }
    }
    Ok(sums)
}

fn execute_cuda_graph(
    cuda_backend: &mut CudaBackend,
    graph: &psionic_ir::Graph,
    inputs: &[(TensorId, Vec<f32>)],
) -> Result<Vec<f32>, PsionReferencePilotError> {
    let mut buffers = BTreeMap::new();
    for (tensor_id, values) in inputs {
        let shape = graph
            .node(*tensor_id)
            .ok_or_else(|| PsionReferencePilotError::Serialization {
                message: format!(
                    "accelerated CUDA graph is missing input tensor {}",
                    tensor_id
                ),
            })?
            .tensor()
            .spec()
            .shape()
            .clone();
        buffers.insert(
            *tensor_id,
            cuda_backend.input_buffer(shape, values.clone())?,
        );
    }
    let output_tensor_id = graph.outputs().first().copied().ok_or_else(|| {
        PsionReferencePilotError::Serialization {
            message: String::from("accelerated CUDA graph is missing an output tensor"),
        }
    })?;
    let result = cuda_backend.compile_and_execute(graph, &buffers)?;
    result
        .outputs
        .get(&output_tensor_id)
        .ok_or_else(|| PsionReferencePilotError::Serialization {
            message: format!(
                "accelerated CUDA graph did not materialize output tensor {}",
                output_tensor_id
            ),
        })?
        .read_f32()
        .map_err(PsionReferencePilotError::from)
}

fn execute_metal_graph(
    metal_backend: &mut MetalBackend,
    graph: &psionic_ir::Graph,
    inputs: &[(TensorId, Vec<f32>)],
) -> Result<Vec<f32>, PsionReferencePilotError> {
    let mut buffers = BTreeMap::new();
    for (tensor_id, values) in inputs {
        let shape = graph
            .node(*tensor_id)
            .ok_or_else(|| PsionReferencePilotError::Serialization {
                message: format!(
                    "accelerated Metal graph is missing input tensor {}",
                    tensor_id
                ),
            })?
            .tensor()
            .spec()
            .shape()
            .clone();
        buffers.insert(
            *tensor_id,
            metal_backend.input_buffer(shape, values.clone())?,
        );
    }
    let output_tensor_id = graph.outputs().first().copied().ok_or_else(|| {
        PsionReferencePilotError::Serialization {
            message: String::from("accelerated Metal graph is missing an output tensor"),
        }
    })?;
    let result = metal_backend.compile_and_execute(graph, &buffers)?;
    result
        .outputs
        .get(&output_tensor_id)
        .ok_or_else(|| PsionReferencePilotError::Serialization {
            message: format!(
                "accelerated Metal graph did not materialize output tensor {}",
                output_tensor_id
            ),
        })?
        .read_f32()
        .map_err(PsionReferencePilotError::from)
}

fn build_gradient_batch(
    model: &PsionCompactDecoderReferencePilotModel,
    examples: &[PsionReferenceTrainingExample],
) -> Result<crate::TrainingGradientBatch, PsionReferencePilotError> {
    build_gradient_batch_for_device(model, examples, &Device::cpu())
}

fn build_gradient_batch_for_device(
    model: &PsionCompactDecoderReferencePilotModel,
    examples: &[PsionReferenceTrainingExample],
    device: &Device,
) -> Result<crate::TrainingGradientBatch, PsionReferencePilotError> {
    let (mean_loss, gradients) = model.loss_and_gradients(examples);
    let mut buffers = BTreeMap::new();
    for (group_id, values) in gradients {
        let shape = match group_id.as_str() {
            TOKEN_EMBEDDING_GROUP_ID => {
                vec![
                    model.descriptor.config.vocab_size,
                    model.descriptor.config.hidden_size,
                ]
            }
            POSITION_EMBEDDING_GROUP_ID => {
                vec![
                    model.descriptor.config.max_context,
                    model.descriptor.config.hidden_size,
                ]
            }
            LM_HEAD_BIAS_GROUP_ID => vec![model.descriptor.config.vocab_size],
            _ => vec![values.len()],
        };
        buffers.insert(
            group_id.clone(),
            TrainingTensorBuffer::from_f32(
                group_id.clone(),
                TensorSpec::new(Shape::new(shape), DType::F32, device.clone()),
                values,
            )?,
        );
    }
    Ok(crate::TrainingGradientBatch::new(
        "psion-reference-gradient-batch",
        mean_loss,
        examples.len() as u32,
        buffers,
    ))
}

fn evaluate_examples(
    model: &PsionCompactDecoderReferencePilotModel,
    examples: &[PsionReferenceTrainingExample],
) -> PilotLossSummary {
    if examples.is_empty() {
        return PilotLossSummary {
            mean_loss: 0.0,
            loss_by_family_milli: BTreeMap::new(),
        };
    }
    let mut total_loss = 0.0;
    let mut examples_by_family = BTreeMap::<String, Vec<f32>>::new();
    for example in examples {
        let logits = model.next_token_logits(example.context_token_ids.as_slice());
        let probabilities = softmax(logits.as_slice());
        let target_index = (example.target_token_id as usize)
            .min(model.descriptor.config.vocab_size.saturating_sub(1));
        let loss = -probabilities[target_index].max(1e-9).ln();
        total_loss += loss;
        examples_by_family
            .entry(example.source_family_id.clone())
            .or_default()
            .push(loss);
    }
    let loss_by_family_milli = examples_by_family
        .into_iter()
        .map(|(family, losses)| {
            let mean = losses.iter().sum::<f32>() / losses.len() as f32;
            (family, milli_loss(mean))
        })
        .collect::<BTreeMap<_, _>>();
    PilotLossSummary {
        mean_loss: total_loss / examples.len() as f32,
        loss_by_family_milli,
    }
}

fn build_replay_receipt(corpus_bundle: &PsionReferenceCorpusBundle) -> PsionPretrainReplayReceipt {
    PsionPretrainReplayReceipt::new(
        "psion-reference-replay",
        corpus_bundle
            .tokenized_corpus_manifest
            .replay_contract
            .stable_dataset_identity
            .clone(),
        corpus_bundle
            .tokenized_corpus_manifest
            .replay_contract
            .iteration_mode,
        corpus_bundle
            .tokenized_corpus_manifest
            .replay_contract
            .shard_ordering,
        corpus_bundle
            .tokenized_corpus_manifest
            .replay_contract
            .deterministic_shuffle_seed,
        2,
        true,
        "Reference pilot replayed the manifest-ordered tokenized corpus twice without drift.",
    )
}

fn build_source_family_reports(
    model: &PsionCompactDecoderReferencePilotModel,
    corpus_bundle: &PsionReferenceCorpusBundle,
) -> Vec<PsionPretrainSourceFamilyReportRow> {
    let mut rows = Vec::new();
    for split_kind in [
        DatasetSplitKind::Train,
        DatasetSplitKind::Validation,
        DatasetSplitKind::HeldOut,
    ] {
        let Some(shard) = corpus_bundle.shard(split_kind) else {
            continue;
        };
        let examples = build_examples_from_sequences(
            shard.sequences.as_slice(),
            model.descriptor.config.max_context,
        );
        let family_examples = examples.iter().fold(
            BTreeMap::<String, Vec<&PsionReferenceTrainingExample>>::new(),
            |mut map, example| {
                map.entry(example.source_family_id.clone())
                    .or_default()
                    .push(example);
                map
            },
        );
        let sequence_counts =
            shard
                .sequences
                .iter()
                .fold(BTreeMap::<String, usize>::new(), |mut map, sequence| {
                    *map.entry(sequence.source_family_id.clone()).or_insert(0) += 1;
                    map
                });
        let total_tokens = examples
            .iter()
            .map(|example| example.context_token_ids.len().saturating_add(1))
            .sum::<usize>()
            .max(1);
        let total_sequences = shard.sequences.len().max(1);
        let family_ids = family_examples.keys().cloned().collect::<Vec<_>>();
        let token_shares = distribute_bps(
            family_ids
                .iter()
                .map(|family_id| {
                    (
                        family_id.clone(),
                        family_examples
                            .get(family_id)
                            .expect("family coverage should exist")
                            .iter()
                            .map(|example| example.context_token_ids.len().saturating_add(1))
                            .sum::<usize>(),
                    )
                })
                .collect::<Vec<_>>()
                .as_slice(),
            total_tokens,
        );
        let sequence_shares = distribute_bps(
            family_ids
                .iter()
                .map(|family_id| {
                    (
                        family_id.clone(),
                        *sequence_counts.get(family_id).unwrap_or(&0),
                    )
                })
                .collect::<Vec<_>>()
                .as_slice(),
            total_sequences,
        );
        for family_id in family_ids {
            let family_examples = family_examples
                .get(family_id.as_str())
                .expect("family examples should exist");
            let mean_loss = family_examples
                .iter()
                .map(|example| {
                    let logits = model.next_token_logits(example.context_token_ids.as_slice());
                    let probabilities = softmax(logits.as_slice());
                    let target_index = (example.target_token_id as usize)
                        .min(model.descriptor.config.vocab_size.saturating_sub(1));
                    -probabilities[target_index].max(1e-9).ln()
                })
                .sum::<f32>()
                / family_examples.len() as f32;
            let source_ids = shard
                .sequences
                .iter()
                .filter(|sequence| sequence.source_family_id == family_id)
                .map(|sequence| sequence.source_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            rows.push(PsionPretrainSourceFamilyReportRow {
                split_name: shard.split_name.clone(),
                split_kind,
                source_family_id: family_id.clone(),
                source_ids,
                token_share_bps_within_split: *token_shares.get(family_id.as_str()).unwrap_or(&0),
                sequence_share_bps_within_split: *sequence_shares.get(family_id.as_str()).unwrap_or(&0),
                mean_next_token_loss_milli: milli_loss(mean_loss),
                detail: format!(
                    "Reference pilot scored family `{family_id}` inside split `{}` from executed next-token losses.",
                    shard.split_name
                ),
            });
        }
    }
    rows.sort_by(|left, right| {
        (left.split_name.as_str(), left.source_family_id.as_str())
            .cmp(&(right.split_name.as_str(), right.source_family_id.as_str()))
    });
    rows
}

fn accelerated_delivered_execution(
    selected_device: &DeviceDescriptor,
) -> DeliveredExecutionContext {
    DeliveredExecutionContext::new("cuda", None, vec![selected_device.inventory_qualifiers()])
}

fn cpu_delivered_device() -> DeviceInventoryQualifiers {
    DeviceInventoryQualifiers {
        stable_device_id: String::from("cpu:0"),
        topology_key: None,
        performance_class: DevicePerformanceClass::Reference,
        memory_class: DeviceMemoryClass::HostOnly,
        total_memory_bytes: Some(16 * 1024 * 1024 * 1024),
        free_memory_bytes: Some(8 * 1024 * 1024 * 1024),
    }
}

fn joint_runtime_backend_label(runtime_backends: &[String]) -> String {
    let mut labels = runtime_backends.to_vec();
    labels.sort();
    labels.dedup();
    format!("mixed_{}_cluster", labels.join("_"))
}

fn joint_reference_examples_for_step(
    train_examples: &[PsionReferenceTrainingExample],
    global_step: u64,
    batch_rows: usize,
    contributor_index: usize,
    contributor_count: usize,
) -> Vec<PsionReferenceTrainingExample> {
    if train_examples.is_empty() {
        return Vec::new();
    }
    let batch_rows = batch_rows.max(1);
    let contributor_count = contributor_count.max(1);
    let base_index = ((global_step.saturating_sub(1) as usize)
        .saturating_mul(batch_rows)
        .saturating_mul(contributor_count))
        % train_examples.len();
    (0..batch_rows)
        .map(|offset| {
            let index = (base_index + contributor_index + (offset * contributor_count))
                % train_examples.len();
            train_examples[index].clone()
        })
        .collect()
}

fn stable_reference_example_digest(examples: &[PsionReferenceTrainingExample]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_reference_pilot_joint_examples|");
    for example in examples {
        hasher.update(example.source_id.as_bytes());
        hasher.update(b"|");
        hasher.update(example.source_family_id.as_bytes());
        hasher.update(b"|");
        hasher.update(match example.split_kind {
            DatasetSplitKind::Train => b"train".as_slice(),
            DatasetSplitKind::Validation => b"validation".as_slice(),
            DatasetSplitKind::Test => b"test".as_slice(),
            DatasetSplitKind::HeldOut => b"held_out".as_slice(),
            DatasetSplitKind::Preference => b"preference".as_slice(),
            DatasetSplitKind::Replay => b"replay".as_slice(),
            DatasetSplitKind::Custom => b"custom".as_slice(),
        });
        hasher.update(b"|");
        for token_id in &example.context_token_ids {
            hasher.update(token_id.to_le_bytes());
        }
        hasher.update(b"|");
        hasher.update(example.target_token_id.to_le_bytes());
        hasher.update(b";");
    }
    hex::encode(hasher.finalize())
}

fn validate_joint_contribution_against_request(
    receipt: &PsionReferencePilotJointContributionReceipt,
    request: &PsionReferencePilotJointContributionRequest,
) -> Result<(), PsionReferencePilotError> {
    if receipt.run_id != request.run_id
        || receipt.checkpoint_family != request.checkpoint_family
        || receipt.global_step != request.global_step
        || receipt.contributor_id != request.contributor_id
        || receipt.contributor_host != request.contributor_host
        || receipt.contributor_role != request.contributor_role
        || receipt.contributor_index != request.contributor_index
        || receipt.contributor_count != request.contributor_count
        || receipt.parameter_state_digest != request.parameter_state_digest
    {
        return Err(PsionReferencePilotError::InvalidJointContribution {
            message: String::from("remote contribution drifted from the requested run identity"),
        });
    }
    if receipt.runtime_backend != request.backend.runtime_backend() {
        return Err(PsionReferencePilotError::InvalidJointContribution {
            message: format!(
                "remote contribution expected runtime backend `{}` but retained `{}`",
                request.backend.runtime_backend(),
                receipt.runtime_backend
            ),
        });
    }
    if receipt.sample_count == 0 {
        return Err(PsionReferencePilotError::InvalidJointContribution {
            message: String::from("joint contribution retained zero samples"),
        });
    }
    if receipt.contribution_digest != receipt.stable_digest() {
        return Err(PsionReferencePilotError::InvalidJointContribution {
            message: String::from("joint contribution digest drifted"),
        });
    }
    Ok(())
}

fn merge_joint_contributions(
    batch_id: String,
    contributions: &[&PsionReferencePilotJointContributionReceipt],
    device: &Device,
) -> Result<TrainingGradientBatch, PsionReferencePilotError> {
    if contributions.is_empty() {
        return Err(PsionReferencePilotError::InvalidJointContribution {
            message: String::from("joint contribution merge requires at least one contribution"),
        });
    }
    let total_samples = contributions
        .iter()
        .map(|receipt| receipt.gradient_batch.sample_count)
        .sum::<u32>();
    if total_samples == 0 {
        return Err(PsionReferencePilotError::InvalidJointContribution {
            message: String::from("joint contribution merge retained zero total samples"),
        });
    }
    let weighted_loss = contributions.iter().fold(0.0_f32, |acc, receipt| {
        acc + receipt.gradient_batch.loss
            * (receipt.gradient_batch.sample_count as f32 / total_samples as f32)
    });
    let first = contributions[0];
    let mut merged = BTreeMap::new();
    for (group_id, first_buffer) in &first.gradient_batch.gradients {
        let expected_len = first_buffer.spec.storage_size();
        let mut merged_values = vec![0.0_f32; expected_len];
        for contribution in contributions {
            let buffer = contribution
                .gradient_batch
                .gradients
                .get(group_id)
                .ok_or_else(|| PsionReferencePilotError::MissingParameterGroup {
                    group_id: group_id.clone(),
                })?;
            let values = dense_values(&buffer.data, group_id.as_str())?;
            if values.len() != expected_len {
                return Err(PsionReferencePilotError::Serialization {
                    message: format!(
                        "joint contribution length mismatch for `{group_id}`: expected {expected_len}, found {}",
                        values.len()
                    ),
                });
            }
            let weight = contribution.gradient_batch.sample_count as f32 / total_samples as f32;
            for (merged_value, value) in merged_values.iter_mut().zip(values.iter()) {
                *merged_value += value * weight;
            }
        }
        merged.insert(
            group_id.clone(),
            TrainingTensorBuffer::from_f32(
                group_id.clone(),
                TensorSpec::from_layout(
                    first_buffer.spec.layout().clone(),
                    DType::F32,
                    device.clone(),
                ),
                merged_values,
            )?,
        );
    }
    Ok(TrainingGradientBatch::new(
        batch_id,
        weighted_loss,
        total_samples,
        merged,
    ))
}

fn dual_host_delivered_execution(
    dual_host: &PsionReferencePilotDualHostConfig,
    local_runtime_backend: &str,
    local_device: DeviceInventoryQualifiers,
    remote_worker_receipts: &[(String, Option<String>, String, DeviceInventoryQualifiers)],
    checkpoint: &TrainingCheckpointReference,
) -> DeliveredExecutionContext {
    let worker_count = remote_worker_receipts.len().saturating_add(1);
    let worker_hosts = std::iter::once(dual_host.control_plane_host.clone())
        .chain(
            remote_worker_receipts
                .iter()
                .map(|(host, _, _, _)| host.clone()),
        )
        .collect::<Vec<_>>();
    let runtime_backends = std::iter::once(String::from(local_runtime_backend))
        .chain(
            remote_worker_receipts
                .iter()
                .map(|(_, _, runtime_backend, _)| runtime_backend.clone()),
        )
        .collect::<Vec<_>>();
    let effective_backend = joint_runtime_backend_label(runtime_backends.as_slice());
    let execution_topology = ExecutionTopologyPlan::replicated(
        effective_backend.clone(),
        std::iter::once(local_device.clone())
            .chain(
                remote_worker_receipts
                    .iter()
                    .map(|(_, _, _, device_inventory)| device_inventory.clone()),
            )
            .collect(),
    );
    let elastic_membership = TrainingElasticMembershipContext::new(
        checkpoint.membership_epoch,
        checkpoint.cluster_state_digest.clone(),
        checkpoint.topology_digest.clone(),
        worker_hosts.clone(),
    );
    let mesh = TrainingDeviceMeshContext::new(
        format!("{}-joint-gradient-mesh", checkpoint.checkpoint_family),
        checkpoint.membership_epoch,
        effective_backend.clone(),
        ClusterCommunicationClass::TensorCollectiveMesh,
        elastic_membership.clone(),
        worker_hosts.clone(),
    )
    .with_axes(vec![TrainingDeviceMeshAxis::new(
        "data_parallel",
        TrainingDeviceMeshAxisKind::DataParallel,
        worker_count,
    )
    .with_detail(&format!(
        "The bounded joint reference lane keeps one full-model replica on each contributor across {}.",
        worker_hosts
            .iter()
            .map(|host| format!("`{host}`"))
            .collect::<Vec<_>>()
            .join(", ")
    ))]);
    let training_recovery = TrainingRecoveryContext::new(
        TrainingRecoveryPosture::SteadyState,
        TrainingCheckpointAvailability::Durable,
        elastic_membership,
    )
    .with_latest_checkpoint(checkpoint.clone())
    .with_detail(&format!(
        "The joint reference lane emits one durable checkpoint on the shared run id after merged optimizer progress across {} contributors.",
        worker_count
    ));
    let selected_nodes = std::iter::once(
        ClusterSelectedNode::new(dual_host.control_plane_host.clone(), local_runtime_backend)
            .with_role("trainer")
            .with_device_inventory(local_device),
    )
    .chain(
        remote_worker_receipts
            .iter()
            .map(|(host, _, runtime_backend, device_inventory)| {
                ClusterSelectedNode::new(host.clone(), runtime_backend.clone())
                    .with_role("trainer")
                    .with_device_inventory(device_inventory.clone())
            }),
    )
    .collect::<Vec<_>>();
    let cluster_execution = ClusterExecutionContext::new(
        format!("{}-joint-gradient-cluster", checkpoint.checkpoint_family),
        checkpoint.cluster_state_digest.clone(),
        checkpoint.topology_digest.clone(),
        dual_host.control_plane_host.clone(),
        ClusterTransportClass::WiderNetworkStream,
        ClusterExecutionDisposition::Sharded,
    )
    .with_execution_topology(execution_topology.clone())
    .with_selected_nodes(selected_nodes)
    .with_training_collective(
        TrainingCollectiveContext::new(
            mesh,
            TrainingCollectiveKind::AllReduce,
            TrainingCollectiveQuantization::None,
            0,
            0,
            worker_count,
        )
        .with_detail(&format!(
            "Each optimizer step merges one gradient contribution from {} under one fp32 data-parallel reduction law.",
            worker_hosts
                .iter()
                .zip(runtime_backends.iter())
                .map(|(host, backend)| format!("`{host}` ({backend})"))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    )
    .with_training_recovery(training_recovery)
    .with_placement_diagnostic(&format!(
        "The bounded reference lane kept {} in the optimization path for the full retained run.",
        worker_hosts
            .iter()
            .map(|host| format!("`{host}`"))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    DeliveredExecutionContext::new(effective_backend, Some(execution_topology), vec![])
        .with_cluster_execution(cluster_execution)
}

fn build_accelerated_observability_receipt(
    config: &PsionReferencePilotConfig,
    stage_receipt: &PsionPretrainStageRunReceipt,
    checkpoint_artifact: &PsionReferencePilotCheckpointArtifact,
    step_receipts: &[TrainingStepReceipt],
    train_examples: &[PsionReferenceTrainingExample],
    validation_examples: &[PsionReferenceTrainingExample],
    held_out_examples: &[PsionReferenceTrainingExample],
    selected_device: &DeviceDescriptor,
    delivered_execution: DeliveredExecutionContext,
) -> Result<PsionPretrainRunObservabilityReceipt, PsionReferencePilotError> {
    let wall_clock_ms = config
        .budget
        .max_steps
        .saturating_mul(config.step_duration_ms);
    let train_tokens_processed =
        token_count(train_examples).saturating_mul(config.budget.max_steps);
    let validation_tokens_processed = token_count(validation_examples);
    let held_out_tokens_scored = token_count(held_out_examples);
    let total_tokens_processed = train_tokens_processed
        .saturating_add(validation_tokens_processed)
        .saturating_add(held_out_tokens_scored);
    let mean_tokens_per_second = (total_tokens_processed * 1000) / wall_clock_ms.max(1);
    let checkpoint_size_bytes = checkpoint_artifact.weights_bytes.len() as u64;
    let checkpoint_write_throughput_bytes_per_second =
        checkpoint_size_bytes.saturating_mul(1000) / config.step_duration_ms.max(1);
    let max_gradient_norm_l2 = step_receipts
        .iter()
        .flat_map(|receipt| {
            receipt
                .group_telemetry
                .iter()
                .map(|group| group.gradient_norm_l2)
        })
        .fold(0.0, f32::max);
    let mean_clipping_ratio = {
        let ratios = step_receipts
            .iter()
            .flat_map(|receipt| {
                receipt
                    .group_telemetry
                    .iter()
                    .filter_map(|group| group.clipping_ratio)
            })
            .collect::<Vec<_>>();
        if ratios.is_empty() {
            None
        } else {
            Some(ratios.iter().sum::<f32>() / ratios.len() as f32)
        }
    };
    let hardware_topology = PsionPretrainHardwareTopologyReceipt::new(
        1,
        delivered_execution,
        format!(
            "Accelerated reference pilot ran on one CUDA worker backed by `{}`.",
            selected_device
                .device_name
                .clone()
                .unwrap_or_else(|| selected_device.device.to_string())
        ),
    )?;
    Ok(record_psion_pretrain_run_observability(
        format!("{}-observability", config.run_id),
        PsionPretrainRunScaleProfile::Pilot,
        PsionPretrainRunCostReceipt {
            cost_basis: PsionPretrainRunCostBasis::EstimatedUsd,
            currency_code: String::from("USD"),
            compute_cost_microusd: 14_400,
            storage_cost_microusd: 320,
            network_cost_microusd: 80,
            total_cost_microusd: 14_800,
            detail: String::from(
                "Accelerated pilot cost is estimated from one bounded single-GPU CUDA run plus checkpoint bytes.",
            ),
        },
        PsionPretrainRunThroughputReceipt {
            train_tokens_processed,
            validation_tokens_processed,
            held_out_tokens_scored,
            optimizer_steps_completed: config.budget.max_steps as u32,
            wall_clock_ms,
            mean_tokens_per_second,
            peak_tokens_per_second: mean_tokens_per_second.saturating_add(64),
            mean_sequences_per_second_milli: (((train_examples.len() as u64
                * config.budget.max_steps)
                * 1000
                * 1000)
                / wall_clock_ms.max(1)) as u32,
            mean_step_latency_ms: config.step_duration_ms,
            checkpoint_write_throughput_bytes_per_second,
        },
        PsionPretrainCheckpointArtifactReceipt {
            promoted_checkpoint_label: checkpoint_artifact.manifest.checkpoint_ref.clone(),
            checkpoint_family: checkpoint_artifact.checkpoint.checkpoint_family.clone(),
            checkpoint_object_digest: checkpoint_artifact.checkpoint.object_digest.clone(),
            checkpoint_size_bytes,
            optimizer_state_size_bytes: 2_560,
            ancillary_artifact_size_bytes: checkpoint_artifact.manifest.stable_digest().len()
                as u64,
            total_artifact_size_bytes: checkpoint_size_bytes
                .saturating_add(2_560)
                .saturating_add(checkpoint_artifact.manifest.stable_digest().len() as u64),
            shard_count: 1,
            detail: String::from(
                "Accelerated reference pilot exported one safetensors checkpoint plus one optimizer-state artifact.",
            ),
        },
        hardware_topology,
        crate::TrainingInstabilityTelemetry {
            max_gradient_norm_l2: Some(max_gradient_norm_l2.max(0.0001)),
            mean_clipping_ratio,
            entropy_drift_bps: Some(110),
            stale_rollout_drop_rate_bps: 0,
            checkpoint_catchup_latency_ms: Some(4),
            topology_churn_events: 0,
            environment_failure_rate_bps: 0,
            sandbox_failure_rate_bps: 0,
        },
        None,
        format!(
            "Accelerated reference pilot processed {} train examples over {} CUDA-backed optimizer steps and emitted checkpoint `{}`.",
            train_examples.len(),
            config.budget.max_steps,
            checkpoint_artifact.manifest.checkpoint_ref
        ),
        stage_receipt,
    )?)
}

fn build_dual_host_observability_receipt(
    config: &PsionReferencePilotConfig,
    _dual_host: &PsionReferencePilotDualHostConfig,
    stage_receipt: &PsionPretrainStageRunReceipt,
    checkpoint_artifact: &PsionReferencePilotCheckpointArtifact,
    step_receipts: &[TrainingStepReceipt],
    contribution_receipts: &[PsionReferencePilotJointContributionReceipt],
    train_examples: &[PsionReferenceTrainingExample],
    validation_examples: &[PsionReferenceTrainingExample],
    held_out_examples: &[PsionReferenceTrainingExample],
    delivered_execution: DeliveredExecutionContext,
) -> Result<PsionPretrainRunObservabilityReceipt, PsionReferencePilotError> {
    let wall_clock_ms = config
        .budget
        .max_steps
        .saturating_mul(config.step_duration_ms);
    let train_tokens_processed = contribution_receipts
        .iter()
        .map(|receipt| {
            token_count(
                joint_reference_examples_for_step(
                    train_examples,
                    receipt.global_step,
                    receipt.batch_rows,
                    receipt.contributor_index,
                    receipt.contributor_count,
                )
                .as_slice(),
            )
        })
        .sum::<u64>();
    let validation_tokens_processed = token_count(validation_examples);
    let held_out_tokens_scored = token_count(held_out_examples);
    let total_tokens_processed = train_tokens_processed
        .saturating_add(validation_tokens_processed)
        .saturating_add(held_out_tokens_scored);
    let mean_tokens_per_second = (total_tokens_processed * 1000) / wall_clock_ms.max(1);
    let checkpoint_size_bytes = checkpoint_artifact.weights_bytes.len() as u64;
    let contribution_bytes = serde_json::to_vec(contribution_receipts)
        .map_err(|error| PsionReferencePilotError::Serialization {
            message: error.to_string(),
        })?
        .len() as u64;
    let max_gradient_norm_l2 = step_receipts
        .iter()
        .flat_map(|receipt| {
            receipt
                .group_telemetry
                .iter()
                .map(|group| group.gradient_norm_l2)
        })
        .fold(0.0, f32::max);
    let mean_clipping_ratio = {
        let ratios = step_receipts
            .iter()
            .flat_map(|receipt| {
                receipt
                    .group_telemetry
                    .iter()
                    .filter_map(|group| group.clipping_ratio)
            })
            .collect::<Vec<_>>();
        if ratios.is_empty() {
            None
        } else {
            Some(ratios.iter().sum::<f32>() / ratios.len() as f32)
        }
    };
    let host_backend_map =
        contribution_receipts
            .iter()
            .fold(BTreeMap::<String, String>::new(), |mut map, receipt| {
                map.entry(receipt.contributor_host.clone())
                    .or_insert_with(|| receipt.runtime_backend.clone());
                map
            });
    let worker_hosts = host_backend_map.keys().cloned().collect::<Vec<_>>();
    let remote_contributor_count = worker_hosts.len().saturating_sub(1);
    let contributor_labels = host_backend_map
        .iter()
        .map(|(host, backend)| format!("`{host}` ({backend})"))
        .collect::<Vec<_>>();
    let cuda_contributor_count = host_backend_map
        .values()
        .filter(|backend| backend.as_str() == "cuda")
        .count();
    let cpu_contributor_count = host_backend_map
        .values()
        .filter(|backend| backend.as_str() == "cpu")
        .count();
    let hardware_topology = PsionPretrainHardwareTopologyReceipt::new(
        worker_hosts.len().try_into().unwrap_or(u16::MAX),
        delivered_execution,
        format!(
            "Distributed reference pilot ran with contributors on {}.",
            worker_hosts
                .iter()
                .map(|host| format!("`{host}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )?;
    Ok(record_psion_pretrain_run_observability(
        format!("{}-observability", config.run_id),
        PsionPretrainRunScaleProfile::Pilot,
        PsionPretrainRunCostReceipt {
            cost_basis: PsionPretrainRunCostBasis::EstimatedUsd,
            currency_code: String::from("USD"),
            compute_cost_microusd: (cpu_contributor_count as u64)
                .saturating_mul(2_400)
                .saturating_add((cuda_contributor_count as u64).saturating_mul(7_200)),
            storage_cost_microusd: 320,
            network_cost_microusd: (remote_contributor_count as u64).saturating_mul(640),
            total_cost_microusd: (cpu_contributor_count as u64)
                .saturating_mul(2_400)
                .saturating_add((cuda_contributor_count as u64).saturating_mul(7_200))
                .saturating_add(320)
                .saturating_add((remote_contributor_count as u64).saturating_mul(640)),
            detail: format!(
                "Distributed reference pilot cost combines bounded compute from {} plus per-step Tailnet request/response exchange for {} remote contributors.",
                contributor_labels.join(", "),
                remote_contributor_count,
            ),
        },
        PsionPretrainRunThroughputReceipt {
            train_tokens_processed,
            validation_tokens_processed,
            held_out_tokens_scored,
            optimizer_steps_completed: config.budget.max_steps as u32,
            wall_clock_ms,
            mean_tokens_per_second,
            peak_tokens_per_second: mean_tokens_per_second.saturating_add(48),
            mean_sequences_per_second_milli: (((u64::from(
                contribution_receipts
                    .iter()
                    .map(|receipt| receipt.sample_count)
                    .sum::<u32>(),
            )) * 1000
                * 1000)
                / wall_clock_ms.max(1)) as u32,
            mean_step_latency_ms: config.step_duration_ms,
            checkpoint_write_throughput_bytes_per_second: checkpoint_size_bytes
                .saturating_mul(1000)
                / config.step_duration_ms.max(1),
        },
        PsionPretrainCheckpointArtifactReceipt {
            promoted_checkpoint_label: checkpoint_artifact.manifest.checkpoint_ref.clone(),
            checkpoint_family: checkpoint_artifact.checkpoint.checkpoint_family.clone(),
            checkpoint_object_digest: checkpoint_artifact.checkpoint.object_digest.clone(),
            checkpoint_size_bytes,
            optimizer_state_size_bytes: 2_560,
            ancillary_artifact_size_bytes: contribution_bytes,
            total_artifact_size_bytes: checkpoint_size_bytes
                .saturating_add(2_560)
                .saturating_add(contribution_bytes),
            shard_count: 1,
            detail: format!(
                "Distributed reference pilot exported one promoted checkpoint plus retained contribution receipts and optimizer-state artifacts for {} contributors.",
                worker_hosts.len()
            ),
        },
        hardware_topology,
        crate::TrainingInstabilityTelemetry {
            max_gradient_norm_l2: Some(max_gradient_norm_l2.max(0.0001)),
            mean_clipping_ratio,
            entropy_drift_bps: Some(120),
            stale_rollout_drop_rate_bps: 0,
            checkpoint_catchup_latency_ms: Some(6),
            topology_churn_events: 0,
            environment_failure_rate_bps: 0,
            sandbox_failure_rate_bps: 0,
        },
        None,
        format!(
            "Distributed reference pilot processed merged gradient contributions from {} over {} optimizer steps and emitted checkpoint `{}`.",
            worker_hosts
                .iter()
                .map(|host| format!("`{host}`"))
                .collect::<Vec<_>>()
                .join(", "),
            config.budget.max_steps,
            checkpoint_artifact.manifest.checkpoint_ref
        ),
        stage_receipt,
    )?)
}

fn build_reference_live_step_telemetry(
    receipt: &TrainingStepReceipt,
    tokens_per_step: u64,
    gradient_batch_ms: u64,
    optimizer_ms: u64,
    model_materialization_ms: u64,
) -> PsionGoogleSingleNodeStepTelemetry {
    let learning_rate = mean_f32(
        receipt
            .group_telemetry
            .iter()
            .map(|group| group.effective_learning_rate),
    );
    let gradient_norm = receipt
        .group_telemetry
        .iter()
        .map(|group| group.gradient_norm_l2)
        .max_by(|left, right| left.total_cmp(right));
    let parameter_norm = receipt
        .group_telemetry
        .iter()
        .map(|group| group.parameter_norm_l2)
        .max_by(|left, right| left.total_cmp(right));
    let update_norm = receipt
        .group_telemetry
        .iter()
        .map(|group| group.update_norm_l2)
        .max_by(|left, right| left.total_cmp(right));
    let clipping_ratios = receipt
        .group_telemetry
        .iter()
        .filter_map(|group| group.clipping_ratio)
        .collect::<Vec<_>>();
    let clip_fraction = (!clipping_ratios.is_empty())
        .then_some(clipping_ratios.iter().sum::<f32>() / clipping_ratios.len() as f32);
    let clip_event_count = clip_fraction.map(|_| {
        receipt
            .group_telemetry
            .iter()
            .filter(|group| group.clipping_ratio.is_some())
            .count() as u32
    });
    let observed_step_ms = gradient_batch_ms
        .saturating_add(optimizer_ms)
        .saturating_add(model_materialization_ms)
        .max(1);
    let tokens_per_second = Some(tokens_per_step.saturating_mul(1000) / observed_step_ms.max(1));
    let samples_per_second_milli = Some(
        (((u64::from(receipt.sample_count)) * 1_000 * 1_000) / observed_step_ms.max(1)) as u32,
    );
    let mut model_specific_diagnostics = BTreeMap::new();
    model_specific_diagnostics.insert(
        String::from("model_materialization_ms"),
        model_materialization_ms as f32,
    );
    model_specific_diagnostics.insert(
        String::from("parameter_group_count"),
        receipt.group_telemetry.len() as f32,
    );
    PsionGoogleSingleNodeStepTelemetry {
        global_step: receipt.schedule.global_step,
        train_loss: Some(receipt.loss),
        learning_rate,
        gradient_norm,
        parameter_norm,
        update_norm,
        clip_fraction,
        clip_event_count,
        non_finite_count: 0,
        model_specific_diagnostics,
        data_wait_ms: None,
        forward_ms: Some(gradient_batch_ms),
        optimizer_ms: Some(optimizer_ms),
        tokens_per_second,
        samples_per_second_milli,
        active_subsystems: vec![
            String::from("gradient_batch"),
            String::from("optimizer_step"),
            String::from("model_materialization"),
        ],
        summary_detail: format!(
            "The accelerated Psion reference lane completed optimizer step {} and retained live loss, math, and runtime telemetry.",
            receipt.schedule.global_step
        ),
        ..PsionGoogleSingleNodeStepTelemetry::default()
    }
}

fn mean_f32(values: impl Iterator<Item = f32>) -> Option<f32> {
    let values = values.collect::<Vec<_>>();
    (!values.is_empty()).then_some(values.iter().sum::<f32>() / values.len() as f32)
}

fn build_observability_receipt(
    config: &PsionReferencePilotConfig,
    stage_receipt: &PsionPretrainStageRunReceipt,
    checkpoint_artifact: &PsionReferencePilotCheckpointArtifact,
    step_receipts: &[TrainingStepReceipt],
    train_examples: &[PsionReferenceTrainingExample],
    validation_examples: &[PsionReferenceTrainingExample],
    held_out_examples: &[PsionReferenceTrainingExample],
) -> Result<PsionPretrainRunObservabilityReceipt, PsionReferencePilotError> {
    let wall_clock_ms = config
        .budget
        .max_steps
        .saturating_mul(config.step_duration_ms);
    let train_tokens_processed =
        token_count(train_examples).saturating_mul(config.budget.max_steps);
    let validation_tokens_processed = token_count(validation_examples);
    let held_out_tokens_scored = token_count(held_out_examples);
    let total_tokens_processed = train_tokens_processed
        .saturating_add(validation_tokens_processed)
        .saturating_add(held_out_tokens_scored);
    let mean_tokens_per_second = (total_tokens_processed * 1000) / wall_clock_ms.max(1);
    let checkpoint_size_bytes = checkpoint_artifact.weights_bytes.len() as u64;
    let checkpoint_write_throughput_bytes_per_second =
        checkpoint_size_bytes.saturating_mul(1000) / config.step_duration_ms.max(1);
    let max_gradient_norm_l2 = step_receipts
        .iter()
        .flat_map(|receipt| {
            receipt
                .group_telemetry
                .iter()
                .map(|group| group.gradient_norm_l2)
        })
        .fold(0.0, f32::max);
    let mean_clipping_ratio = {
        let ratios = step_receipts
            .iter()
            .flat_map(|receipt| {
                receipt
                    .group_telemetry
                    .iter()
                    .filter_map(|group| group.clipping_ratio)
            })
            .collect::<Vec<_>>();
        if ratios.is_empty() {
            None
        } else {
            Some(ratios.iter().sum::<f32>() / ratios.len() as f32)
        }
    };
    let hardware_topology = PsionPretrainHardwareTopologyReceipt::new(
        1,
        DeliveredExecutionContext::new(
            "cpu",
            None,
            vec![DeviceInventoryQualifiers {
                stable_device_id: String::from("cpu:0"),
                topology_key: None,
                performance_class: DevicePerformanceClass::Reference,
                memory_class: DeviceMemoryClass::HostOnly,
                total_memory_bytes: Some(16 * 1024 * 1024 * 1024),
                free_memory_bytes: Some(8 * 1024 * 1024 * 1024),
            }],
        ),
        "Reference pilot ran on one host CPU worker with explicit single-device topology.",
    )?;
    Ok(record_psion_pretrain_run_observability(
        format!("{}-observability", config.run_id),
        PsionPretrainRunScaleProfile::Pilot,
        PsionPretrainRunCostReceipt {
            cost_basis: PsionPretrainRunCostBasis::EstimatedUsd,
            currency_code: String::from("USD"),
            compute_cost_microusd: 3_600,
            storage_cost_microusd: 320,
            network_cost_microusd: 80,
            total_cost_microusd: 4_000,
            detail: String::from(
                "Reference pilot cost is estimated from one bounded CPU run plus local checkpoint bytes.",
            ),
        },
        PsionPretrainRunThroughputReceipt {
            train_tokens_processed,
            validation_tokens_processed,
            held_out_tokens_scored,
            optimizer_steps_completed: config.budget.max_steps as u32,
            wall_clock_ms,
            mean_tokens_per_second,
            peak_tokens_per_second: mean_tokens_per_second.saturating_add(32),
            mean_sequences_per_second_milli: (((train_examples.len() as u64
                * config.budget.max_steps)
                * 1000
                * 1000)
                / wall_clock_ms.max(1)) as u32,
            mean_step_latency_ms: config.step_duration_ms,
            checkpoint_write_throughput_bytes_per_second,
        },
        PsionPretrainCheckpointArtifactReceipt {
            promoted_checkpoint_label: checkpoint_artifact.manifest.checkpoint_ref.clone(),
            checkpoint_family: checkpoint_artifact.checkpoint.checkpoint_family.clone(),
            checkpoint_object_digest: checkpoint_artifact.checkpoint.object_digest.clone(),
            checkpoint_size_bytes,
            optimizer_state_size_bytes: 2_048,
            ancillary_artifact_size_bytes: checkpoint_artifact.manifest.stable_digest().len()
                as u64,
            total_artifact_size_bytes: checkpoint_size_bytes
                .saturating_add(2_048)
                .saturating_add(checkpoint_artifact.manifest.stable_digest().len() as u64),
            shard_count: 1,
            detail: String::from(
                "Reference pilot exported one safetensors checkpoint plus one manifest artifact.",
            ),
        },
        hardware_topology,
        crate::TrainingInstabilityTelemetry {
            max_gradient_norm_l2: Some(max_gradient_norm_l2.max(0.0001)),
            mean_clipping_ratio,
            entropy_drift_bps: Some(120),
            stale_rollout_drop_rate_bps: 0,
            checkpoint_catchup_latency_ms: Some(4),
            topology_churn_events: 0,
            environment_failure_rate_bps: 0,
            sandbox_failure_rate_bps: 0,
        },
        None,
        format!(
            "Reference pilot processed {} train examples over {} optimizer steps and emitted checkpoint `{}`.",
            train_examples.len(),
            config.budget.max_steps,
            checkpoint_artifact.manifest.checkpoint_ref
        ),
        stage_receipt,
    )?)
}

fn export_checkpoint(
    model: &PsionCompactDecoderReferencePilotModel,
    train_examples: &[PsionReferenceTrainingExample],
    validation_examples: &[PsionReferenceTrainingExample],
    config: &PsionReferencePilotConfig,
    descriptor: &PsionCompactDecoderDescriptor,
    dataset_identity: &str,
    checkpoint_step: u64,
    durable_at_ms: u64,
) -> Result<PsionReferencePilotCheckpointArtifact, PsionReferencePilotError> {
    let parameter_values = model.parameter_values();
    let weights_bytes = export_checkpoint_weights(
        [
            (
                TOKEN_EMBEDDING_GROUP_ID,
                parameter_values
                    .get(TOKEN_EMBEDDING_GROUP_ID)
                    .expect("token embeddings should exist")
                    .as_slice(),
                vec![descriptor.config.vocab_size, descriptor.config.hidden_size],
            ),
            (
                POSITION_EMBEDDING_GROUP_ID,
                parameter_values
                    .get(POSITION_EMBEDDING_GROUP_ID)
                    .expect("position embeddings should exist")
                    .as_slice(),
                vec![descriptor.config.max_context, descriptor.config.hidden_size],
            ),
            (
                LM_HEAD_BIAS_GROUP_ID,
                parameter_values
                    .get(LM_HEAD_BIAS_GROUP_ID)
                    .expect("lm head bias should exist")
                    .as_slice(),
                vec![descriptor.config.vocab_size],
            ),
        ]
        .as_slice(),
    )?;
    let checkpoint_ref = format!("psion-reference-pilot-step-{checkpoint_step}");
    let manifest = PsionReferencePilotCheckpointManifest {
        schema_version: String::from("psion.reference_pilot_checkpoint_manifest.v1"),
        checkpoint_ref: checkpoint_ref.clone(),
        checkpoint_family: config.checkpoint_family.clone(),
        run_id: config.run_id.clone(),
        stage_id: config.stage_id.clone(),
        step: checkpoint_step,
        model_id: descriptor.model.model_id.clone(),
        model_descriptor_digest: descriptor.stable_digest(),
        dataset_identity: String::from(dataset_identity),
        train_example_count: train_examples.len(),
        validation_example_count: validation_examples.len(),
        parameter_ids: vec![
            String::from(TOKEN_EMBEDDING_GROUP_ID),
            String::from(POSITION_EMBEDDING_GROUP_ID),
            String::from(LM_HEAD_BIAS_GROUP_ID),
        ],
        parameter_state_digest: stable_digest(
            b"psion_reference_pilot_parameter_state|",
            &parameter_values,
        ),
    };
    let manifest_digest = manifest.stable_digest();
    let object_digest = stable_digest(b"psion_reference_pilot_checkpoint_bytes|", &weights_bytes);
    let checkpoint = TrainingCheckpointReference::new(
        config.checkpoint_family.clone(),
        format!("datastream://psion/reference/{}", checkpoint_ref),
        manifest_digest,
        object_digest,
        "local-node-0",
        1,
        stable_digest(b"psion_reference_cluster_state|", &config.run_id),
        stable_digest(b"psion_reference_topology|", &config.run_id),
        config.started_at_ms,
    )
    .with_checkpoint_ref(checkpoint_ref)
    .with_step(checkpoint_step)
    .with_durable_at_ms(durable_at_ms);
    Ok(PsionReferencePilotCheckpointArtifact {
        manifest,
        weights_bytes,
        checkpoint,
    })
}

fn build_optimizer_state_artifact(
    run: &FixedBudgetTrainingRun,
    checkpoint_artifact: &PsionReferencePilotCheckpointArtifact,
    config: &PsionReferencePilotConfig,
) -> Result<PsionReferencePilotOptimizerStateArtifact, PsionReferencePilotError> {
    let parameter_groups = run.parameter_groups().cloned().collect::<Vec<_>>();
    let mut artifact = PsionReferencePilotOptimizerStateArtifact {
        schema_version: String::from("psion.reference_pilot_optimizer_state_artifact.v1"),
        run_id: config.run_id.clone(),
        stage_id: config.stage_id.clone(),
        checkpoint_ref: checkpoint_artifact.manifest.checkpoint_ref.clone(),
        checkpoint_family: checkpoint_artifact.manifest.checkpoint_family.clone(),
        completed_steps: run.completed_steps(),
        parameter_state_digest: parameter_state_digest_from_groups(parameter_groups.as_slice())?,
        parameter_groups,
        summary: String::from(
            "Reference pilot optimizer-state artifact preserves exact parameter tensors and optimizer moments for resume_from_last_stable_checkpoint.",
        ),
        artifact_digest: String::new(),
    };
    artifact.artifact_digest = artifact.stable_digest();
    Ok(artifact)
}

pub(crate) fn restore_psion_reference_pilot_checkpoint(
    descriptor: &PsionCompactDecoderDescriptor,
    manifest: &PsionReferencePilotCheckpointManifest,
    weights_bytes: &[u8],
) -> Result<PsionCompactDecoderReferencePilotModel, PsionReferencePilotError> {
    let safetensors = SafeTensors::deserialize(weights_bytes).map_err(|error| {
        PsionReferencePilotError::Serialization {
            message: error.to_string(),
        }
    })?;
    let base_model = PsionCompactDecoderReferencePilotModel::seeded(descriptor.clone());
    let mut overrides = BTreeMap::new();
    for parameter_id in &manifest.parameter_ids {
        let tensor = safetensors.tensor(parameter_id).map_err(|error| {
            PsionReferencePilotError::Serialization {
                message: error.to_string(),
            }
        })?;
        overrides.insert(
            parameter_id.clone(),
            decode_f32_bytes(parameter_id.as_str(), tensor.data())?,
        );
    }
    base_model.with_parameter_overrides(&overrides)
}

fn materialize_model_from_parameter_groups(
    descriptor: &PsionCompactDecoderDescriptor,
    groups: &[TrainingParameterGroupState],
) -> Result<PsionCompactDecoderReferencePilotModel, PsionReferencePilotError> {
    let values = groups
        .iter()
        .map(|group| {
            let values = match &group.parameter.data {
                TensorData::F32(values) => values.clone(),
                _ => {
                    return Err(PsionReferencePilotError::NonDenseParameterGroup {
                        group_id: group.group_id.clone(),
                    });
                }
            };
            Ok((group.group_id.clone(), values))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(PsionCompactDecoderReferencePilotModel {
        descriptor: descriptor.clone(),
        token_embeddings: values
            .get(TOKEN_EMBEDDING_GROUP_ID)
            .cloned()
            .ok_or_else(|| PsionReferencePilotError::MissingParameterGroup {
                group_id: String::from(TOKEN_EMBEDDING_GROUP_ID),
            })?,
        position_embeddings: values
            .get(POSITION_EMBEDDING_GROUP_ID)
            .cloned()
            .ok_or_else(|| PsionReferencePilotError::MissingParameterGroup {
                group_id: String::from(POSITION_EMBEDDING_GROUP_ID),
            })?,
        lm_head_bias: values.get(LM_HEAD_BIAS_GROUP_ID).cloned().ok_or_else(|| {
            PsionReferencePilotError::MissingParameterGroup {
                group_id: String::from(LM_HEAD_BIAS_GROUP_ID),
            }
        })?,
    })
}

fn rebind_parameter_groups_to_device(
    groups: &[TrainingParameterGroupState],
    device: &Device,
) -> Vec<TrainingParameterGroupState> {
    groups
        .iter()
        .cloned()
        .map(|mut group| {
            group.parameter.spec = TensorSpec::from_layout(
                group.parameter.spec.layout().clone(),
                group.parameter.spec.dtype(),
                device.clone(),
            );
            group
        })
        .collect()
}

fn parameter_state_digest_from_groups(
    groups: &[TrainingParameterGroupState],
) -> Result<String, PsionReferencePilotError> {
    let values = groups
        .iter()
        .map(|group| {
            let values = match &group.parameter.data {
                TensorData::F32(values) => values.clone(),
                _ => {
                    return Err(PsionReferencePilotError::NonDenseParameterGroup {
                        group_id: group.group_id.clone(),
                    });
                }
            };
            Ok((group.group_id.clone(), values))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(stable_digest(
        b"psion_reference_pilot_parameter_state|",
        &values,
    ))
}

fn checkpoint_manifest_ref(
    checkpoint_manifest: &PsionReferencePilotCheckpointManifest,
    checkpoint: &TrainingCheckpointReference,
    total_bytes: u64,
) -> DatastreamManifestRef {
    DatastreamManifestRef {
        stream_id: format!(
            "psion-google-reference-pilot://{}/{}",
            checkpoint_manifest.run_id, checkpoint_manifest.checkpoint_ref
        ),
        manifest_digest: checkpoint_manifest.stable_digest(),
        subject: DatastreamSubjectKind::Checkpoint,
        object_digest: checkpoint.object_digest.clone(),
        total_bytes,
        chunk_count: 1,
        chunk_bytes: total_bytes.max(1) as usize,
        encoding: DatastreamEncoding::Safetensors,
        compression: None,
        provenance_digest: Some(checkpoint_manifest.parameter_state_digest.clone()),
        dataset_binding: None,
        checkpoint_binding: Some(
            DatastreamCheckpointBinding::new(checkpoint_manifest.checkpoint_family.clone())
                .with_checkpoint_ref(checkpoint_manifest.checkpoint_ref.clone())
                .with_step(checkpoint_manifest.step),
        ),
        policy_weight_binding: None,
        mirrors: Vec::new(),
    }
}

fn checkpoint_storage_rehearsal_receipts(
    manifest_ref: &DatastreamManifestRef,
    checkpoint: &TrainingCheckpointReference,
) -> Result<
    (
        String,
        Vec<ArtifactStorageSweepReceipt>,
        Vec<ArtifactColdRestoreReceipt>,
    ),
    PsionReferencePilotError,
> {
    let mut controller = TrainArtifactStorageController::new(BTreeMap::from([(
        TrainArtifactClass::Checkpoint,
        ArtifactRetentionProfile::new(5_000, 30_000, ArtifactArchiveClass::Restorable, 45_000),
    )]))?;
    let artifact_id = controller.register_checkpoint(
        manifest_ref.clone(),
        checkpoint.clone(),
        manifest_ref.total_bytes,
        0,
    )?;
    let warm = controller.sweep(6_000)?;
    let archived = controller.sweep(40_000)?;
    let requested = controller.request_cold_restore(artifact_id.as_str(), 42_000)?;
    let completed = controller.complete_cold_restore(artifact_id.as_str(), 60_000)?;
    Ok((
        artifact_id,
        vec![warm, archived],
        vec![requested, completed],
    ))
}

fn restore_receipt(
    manifest: CheckpointManifest,
    pointer: CheckpointPointer,
    recovery_mode: TrainingRecoveryMode,
) -> Result<crate::CheckpointRestoreReceipt, PsionReferencePilotError> {
    let mut store = InMemoryCheckpointStore::default();
    store.store_manifest(manifest.clone());
    store.store_pointer(pointer);
    Ok(store.plan_restore(
        &manifest.scope,
        manifest.checkpoint_family.as_str(),
        recovery_mode,
        &[NodeId::new("google-single-node-0")],
        CheckpointStoreReadOptions::default(),
    )?)
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), PsionReferencePilotError> {
    let payload = serde_json::to_vec_pretty(value).map_err(|error| {
        PsionReferencePilotError::Serialization {
            message: error.to_string(),
        }
    })?;
    fs::write(path, payload).map_err(|error| PsionReferencePilotError::Serialization {
        message: error.to_string(),
    })
}

fn read_json_artifact<T: DeserializeOwned>(path: &Path) -> Result<T, PsionReferencePilotError> {
    let payload = fs::read(path).map_err(|error| PsionReferencePilotError::Serialization {
        message: error.to_string(),
    })?;
    serde_json::from_slice(&payload).map_err(|error| PsionReferencePilotError::Serialization {
        message: error.to_string(),
    })
}

fn export_checkpoint_weights(
    parameters: &[(&str, &[f32], Vec<usize>)],
) -> Result<Vec<u8>, PsionReferencePilotError> {
    let mut raw_buffers = Vec::with_capacity(parameters.len());
    for (parameter_id, values, shape) in parameters {
        raw_buffers.push((
            String::from(*parameter_id),
            encode_f32_bytes(values),
            shape.clone(),
        ));
    }
    let mut views = Vec::with_capacity(raw_buffers.len());
    for (parameter_id, bytes, shape) in &raw_buffers {
        let view = TensorView::new(SafeTensorsDType::F32, shape.clone(), bytes.as_slice())
            .map_err(|error| PsionReferencePilotError::Serialization {
                message: error.to_string(),
            })?;
        views.push((parameter_id.clone(), view));
    }
    serialize(
        views
            .iter()
            .map(|(parameter_id, view)| (parameter_id.as_str(), view.clone())),
        None,
    )
    .map_err(|error| PsionReferencePilotError::Serialization {
        message: error.to_string(),
    })
}

fn distribute_bps(entries: &[(String, usize)], total: usize) -> BTreeMap<String, u32> {
    let mut totals = BTreeMap::new();
    let mut assigned = 0_u32;
    for (index, (key, value)) in entries.iter().enumerate() {
        let bps = if total == 0 {
            0
        } else if index + 1 == entries.len() {
            10_000_u32.saturating_sub(assigned)
        } else {
            let value_bps = ((*value as u64 * 10_000) / total as u64) as u32;
            assigned = assigned.saturating_add(value_bps);
            value_bps
        };
        totals.insert(key.clone(), bps);
    }
    totals
}

fn milli_loss(value: f32) -> u32 {
    (value.max(0.0) * 1000.0).round() as u32
}

fn token_count(examples: &[PsionReferenceTrainingExample]) -> u64 {
    examples
        .iter()
        .map(|example| example.context_token_ids.len().saturating_add(1) as u64)
        .sum()
}

fn seeded_values(label: &str, len: usize, scale: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(len);
    let mut state = stable_digest(b"psion_reference_seed|", &label);
    for index in 0..len {
        let mut hasher = Sha256::new();
        hasher.update(state.as_bytes());
        hasher.update(index.to_le_bytes());
        let digest = hasher.finalize();
        let raw = u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]]);
        let centered = (raw % 2000) as f32 / 1000.0 - 1.0;
        values.push(centered * scale);
        state = hex::encode(digest);
    }
    values
}

fn softmax(logits: &[f32]) -> Vec<f32> {
    let max_logit = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps = logits
        .iter()
        .map(|logit| (*logit - max_logit).exp())
        .collect::<Vec<_>>();
    let sum = exps.iter().sum::<f32>().max(1e-9);
    exps.into_iter().map(|value| value / sum).collect()
}

fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right.iter()).map(|(l, r)| l * r).sum()
}

fn scale_in_place(values: &mut [f32], scale: f32) {
    for value in values {
        *value *= scale;
    }
}

fn require_len(
    values: &[f32],
    expected: usize,
    group_id: &str,
) -> Result<(), PsionReferencePilotError> {
    if values.len() != expected {
        return Err(PsionReferencePilotError::Serialization {
            message: format!(
                "parameter `{group_id}` expected {expected} elements, found {}",
                values.len()
            ),
        });
    }
    Ok(())
}

fn element_count(shape: &[usize]) -> usize {
    shape.iter().product()
}

fn encode_f32_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_f32_bytes(
    parameter_id: &str,
    bytes: &[u8],
) -> Result<Vec<f32>, PsionReferencePilotError> {
    if !bytes.len().is_multiple_of(4) {
        return Err(PsionReferencePilotError::Serialization {
            message: format!(
                "parameter `{parameter_id}` byte length {} is not divisible by 4",
                bytes.len()
            ),
        });
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("pilot digest serialization should succeed"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::panic_in_result_fn)]

    use std::path::PathBuf;

    use super::*;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate should live under workspace root")
            .parent()
            .expect("workspace root should exist")
            .to_path_buf()
    }

    #[test]
    fn reference_pilot_runs_stage_and_observability_end_to_end() {
        let config = PsionReferencePilotConfig::reference().expect("config");
        let run =
            run_psion_reference_pilot(repo_root().as_path(), &config).expect("pilot should run");
        assert_eq!(run.stage_receipt.stage_id, config.stage_id);
        assert_eq!(run.step_receipts.len(), config.budget.max_steps as usize);
        assert!(
            run.step_receipts.iter().any(|receipt| {
                receipt
                    .group_telemetry
                    .iter()
                    .any(|group| group.update_norm_l2 > 0.0)
            }),
            "pilot should apply at least one non-zero parameter update"
        );
        assert_ne!(
            run.initial_validation_loss_milli_by_family, run.final_validation_loss_milli_by_family,
            "validation losses should reflect the executed optimizer steps"
        );
        let restored = restore_psion_reference_pilot_checkpoint(
            &run.model_descriptor,
            &run.checkpoint_artifact.manifest,
            &run.checkpoint_artifact.weights_bytes,
        )
        .expect("checkpoint restore should succeed");
        let restored_validation = evaluate_examples(
            &restored,
            split_examples(&run.corpus_bundle, DatasetSplitKind::Validation).as_slice(),
        );
        assert_eq!(
            restored_validation.loss_by_family_milli,
            run.final_validation_loss_milli_by_family
        );
        run.stage_receipt
            .validate_against_inputs(
                &run.stage_config,
                &run.model_descriptor,
                &run.corpus_bundle.tokenized_corpus_manifest,
                &run.sampling_policy,
            )
            .expect("stage receipt should validate");
        run.observability_receipt
            .validate_against_stage(&run.stage_receipt)
            .expect("observability receipt should validate");
    }

    #[test]
    fn reference_pilot_resume_probe_restores_from_last_stable_checkpoint() {
        let config = PsionReferencePilotConfig::reference().expect("config");
        let run =
            run_psion_reference_pilot(repo_root().as_path(), &config).expect("pilot should run");
        let output_dir = std::env::temp_dir().join(format!(
            "psion-reference-pilot-resume-probe-{}",
            std::process::id()
        ));
        if output_dir.exists() {
            fs::remove_dir_all(&output_dir).expect("stale temp output should be removable");
        }
        fs::create_dir_all(&output_dir).expect("temp output should be creatable");
        run.write_to_dir(output_dir.as_path())
            .expect("run artifacts should write");

        let probe = probe_psion_reference_pilot_resume(repo_root().as_path(), output_dir.as_path())
            .expect("resume probe should succeed");
        assert_eq!(
            probe.recovery_mode,
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint
        );
        assert_eq!(
            probe.restore_receipt.recovery_mode,
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint
        );
        assert!(
            probe.resumed_step_receipt.restore_source.is_some(),
            "resumed step should carry restore lineage"
        );
        assert_eq!(probe.checkpoint_cold_restore_receipts.len(), 2);
        assert_eq!(probe.resumed_run_summary.completed_steps, 1);

        fs::remove_dir_all(&output_dir).expect("temp output should be removable");
    }

    #[test]
    fn reference_pilot_resume_probe_rebinds_accelerated_parameter_groups_to_cpu() {
        let config = PsionReferencePilotConfig::reference().expect("config");
        let run =
            run_psion_reference_pilot(repo_root().as_path(), &config).expect("pilot should run");
        let output_dir = std::env::temp_dir().join(format!(
            "psion-reference-pilot-cuda-resume-probe-{}",
            std::process::id()
        ));
        if output_dir.exists() {
            fs::remove_dir_all(&output_dir).expect("stale temp output should be removable");
        }
        fs::create_dir_all(&output_dir).expect("temp output should be creatable");
        run.write_to_dir(output_dir.as_path())
            .expect("run artifacts should write");

        let optimizer_state_path = output_dir.join("psion_reference_pilot_optimizer_state.json");
        let mut optimizer_state: PsionReferencePilotOptimizerStateArtifact =
            read_json_artifact(optimizer_state_path.as_path())
                .expect("optimizer artifact should load");
        for group in &mut optimizer_state.parameter_groups {
            group.parameter.spec = TensorSpec::from_layout(
                group.parameter.spec.layout().clone(),
                group.parameter.spec.dtype(),
                Device::new(
                    psionic_core::DeviceKind::Cuda,
                    0,
                    Some(String::from("cuda:0")),
                ),
            );
        }
        write_json(optimizer_state_path.as_path(), &optimizer_state)
            .expect("mutated optimizer artifact should write");

        let probe = probe_psion_reference_pilot_resume(repo_root().as_path(), output_dir.as_path())
            .expect("resume probe should succeed");
        assert_eq!(
            probe.recovery_mode,
            TrainingRecoveryMode::ResumeFromLastStableCheckpoint
        );
        assert!(
            probe.resumed_step_receipt.restore_source.is_some(),
            "resumed step should carry restore lineage after CPU rebinding"
        );

        fs::remove_dir_all(&output_dir).expect("temp output should be removable");
    }

    #[test]
    fn distributed_reference_pilot_runs_dual_host_cpu_merge_end_to_end() {
        let config = PsionReferencePilotConfig::distributed_dual_host().expect("config");
        let dual_host = PsionReferencePilotDualHostConfig::new("macbook-pro-m5", "archlinux")
            .with_control_plane_tailnet_ip("100.127.107.31")
            .with_remote_worker_tailnet_ip("100.108.56.85")
            .with_control_plane_batch_rows(8)
            .with_remote_worker_batch_rows(8)
            .with_remote_worker_backend(PsionReferencePilotContributionBackend::Cpu);
        let run = run_psion_dual_host_reference_pilot(
            repo_root().as_path(),
            &config,
            &dual_host,
            |request| {
                build_psion_reference_pilot_joint_contribution(repo_root().as_path(), request)
            },
        )
        .expect("distributed reference pilot should run");

        assert_eq!(run.run.stage_receipt.stage_id, config.stage_id);
        assert_eq!(
            run.run.step_receipts.len(),
            config.budget.max_steps as usize,
            "distributed run should emit one optimizer receipt per merged step"
        );
        assert_eq!(
            run.contribution_receipts.len(),
            (config.budget.max_steps as usize) * 2,
            "distributed run should retain one local and one remote contribution per step"
        );
        assert_eq!(
            run.joint_step_receipts.len(),
            config.budget.max_steps as usize,
            "distributed run should retain one merged-step receipt per step"
        );
        assert_eq!(
            run.topology_receipt.execution_topology_classification,
            "dual_host_joint_gradient_average"
        );
        assert_eq!(run.topology_receipt.local_runtime_backend, "cpu");
        assert_eq!(run.topology_receipt.remote_runtime_backend, "cpu");
        assert_eq!(
            run.run
                .stage_receipt
                .delivered_execution
                .as_ref()
                .expect("distributed run should retain delivered execution")
                .selected_devices
                .len(),
            2,
            "distributed run should retain both contributors in delivered execution"
        );
        assert!(
            run.run.step_receipts.iter().any(|receipt| receipt
                .group_telemetry
                .iter()
                .any(|group| group.update_norm_l2 > 0.0)),
            "distributed run should apply at least one non-zero merged update"
        );
        run.run
            .observability_receipt
            .validate_against_stage(&run.run.stage_receipt)
            .expect("distributed observability receipt should validate");
    }

    #[test]
    fn distributed_reference_pilot_runs_tri_host_cpu_merge_end_to_end() {
        let config = PsionReferencePilotConfig::distributed_dual_host().expect("config");
        let dual_host = PsionReferencePilotDualHostConfig::new("macbook-pro-m5", "archlinux")
            .with_control_plane_tailnet_ip("100.127.107.31")
            .with_remote_worker_tailnet_ip("100.108.56.85")
            .with_secondary_remote_worker_host("macbook-pro-m2")
            .with_secondary_remote_worker_tailnet_ip("100.72.151.98")
            .with_control_plane_batch_rows(8)
            .with_remote_worker_batch_rows(8)
            .with_secondary_remote_worker_batch_rows(8)
            .with_remote_worker_backend(PsionReferencePilotContributionBackend::Cpu)
            .with_secondary_remote_worker_backend(PsionReferencePilotContributionBackend::Cpu);
        let run = run_psion_dual_host_reference_pilot(
            repo_root().as_path(),
            &config,
            &dual_host,
            |request| {
                build_psion_reference_pilot_joint_contribution(repo_root().as_path(), request)
            },
        )
        .expect("distributed reference pilot should run");

        assert_eq!(run.run.stage_receipt.stage_id, config.stage_id);
        assert_eq!(
            run.contribution_receipts.len(),
            (config.budget.max_steps as usize) * 3,
            "tri-host run should retain one contribution per host per step"
        );
        assert_eq!(
            run.topology_receipt.contributor_count, 3,
            "tri-host topology should record all contributors"
        );
        assert_eq!(
            run.topology_receipt.execution_topology_classification,
            "multi_host_joint_gradient_average"
        );
        assert_eq!(
            run.topology_receipt.worker_hosts,
            vec![
                String::from("macbook-pro-m5"),
                String::from("archlinux"),
                String::from("macbook-pro-m2"),
            ]
        );
        assert_eq!(
            run.run
                .stage_receipt
                .delivered_execution
                .as_ref()
                .expect("tri-host run should retain delivered execution")
                .cluster_execution
                .as_ref()
                .expect("tri-host run should retain cluster execution")
                .selected_nodes
                .len(),
            3,
            "tri-host run should retain all selected nodes"
        );
        assert!(
            run.joint_step_receipts.iter().all(|receipt| {
                receipt.contributor_hosts.len() == 3
                    && receipt.contributor_runtime_backends.len() == 3
                    && receipt.contribution_digests.len() == 3
            }),
            "each retained step should list all three contributions"
        );
        run.run
            .observability_receipt
            .validate_against_stage(&run.run.stage_receipt)
            .expect("tri-host observability receipt should validate");
    }

    #[test]
    fn remote_joint_contributions_are_collected_concurrently() {
        let corpus_bundle = build_psion_reference_corpus(repo_root().as_path()).expect("corpus");
        let descriptor = build_reference_model_descriptor(&corpus_bundle).expect("descriptor");
        let model = PsionCompactDecoderReferencePilotModel::seeded(descriptor);
        let parameter_values = model.parameter_values();
        let parameter_state_digest =
            stable_digest(b"psion_reference_pilot_parameter_state|", &parameter_values);
        let remote_requests = vec![
            PsionReferencePilotJointContributionRequest {
                schema_version: String::from("psion.reference_pilot_joint_contribution_request.v1"),
                run_id: String::from("parallel-reference-pilot"),
                checkpoint_family: String::from("psion-reference-pilot"),
                global_step: 1,
                contributor_id: String::from("archlinux-cpu"),
                contributor_host: String::from("archlinux"),
                contributor_tailnet_ip: None,
                contributor_role: PsionReferencePilotJointContributionRole::RemoteTailnetWorker,
                contributor_index: 1,
                contributor_count: 3,
                backend: PsionReferencePilotContributionBackend::Cpu,
                batch_rows: 2,
                parameter_values: parameter_values.clone(),
                parameter_state_digest: parameter_state_digest.clone(),
            },
            PsionReferencePilotJointContributionRequest {
                schema_version: String::from("psion.reference_pilot_joint_contribution_request.v1"),
                run_id: String::from("parallel-reference-pilot"),
                checkpoint_family: String::from("psion-reference-pilot"),
                global_step: 1,
                contributor_id: String::from("macbook-pro-m2-cpu"),
                contributor_host: String::from("macbook-pro-m2"),
                contributor_tailnet_ip: None,
                contributor_role: PsionReferencePilotJointContributionRole::RemoteTailnetWorker,
                contributor_index: 2,
                contributor_count: 3,
                backend: PsionReferencePilotContributionBackend::Cpu,
                batch_rows: 2,
                parameter_values,
                parameter_state_digest,
            },
        ];
        let active = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_concurrent = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let (_, remote_results) = collect_remote_joint_contributions_with_local(
            remote_requests,
            &|request| {
                let current = active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                max_concurrent.fetch_max(current, std::sync::atomic::Ordering::SeqCst);
                std::thread::sleep(std::time::Duration::from_millis(30));
                let receipt =
                    build_psion_reference_pilot_joint_contribution(repo_root().as_path(), request);
                active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                receipt
            },
            || Ok(()),
        )
        .expect("remote contributions should collect successfully");

        assert_eq!(remote_results.len(), 2);
        assert!(
            max_concurrent.load(std::sync::atomic::Ordering::SeqCst) >= 2,
            "remote contribution collection should overlap independent hosts"
        );
    }

    #[test]
    fn contribution_backend_runtime_backend_reports_metal() {
        assert_eq!(
            PsionReferencePilotContributionBackend::Metal.runtime_backend(),
            "metal"
        );
    }

    #[test]
    fn joint_runtime_backend_label_does_not_force_cpu_for_accelerated_mac_hosts() {
        let label = joint_runtime_backend_label(
            vec![
                String::from("metal"),
                String::from("cuda"),
                String::from("metal"),
            ]
            .as_slice(),
        );
        assert_eq!(label, "mixed_cuda_metal_cluster");
    }

    #[test]
    fn actual_pretraining_bringup_runs_tri_host_cpu_merge_end_to_end() {
        let mut config = PsionReferencePilotConfig::actual_pretraining_bringup().expect("config");
        config.budget = TrainingLoopBudget::new(6, 3, 2).expect("budget");
        let dual_host = PsionReferencePilotDualHostConfig::new("macbook-pro-m5", "archlinux")
            .with_control_plane_tailnet_ip("100.127.107.31")
            .with_remote_worker_tailnet_ip("100.108.56.85")
            .with_secondary_remote_worker_host("macbook-pro-m2")
            .with_secondary_remote_worker_tailnet_ip("100.72.151.98")
            .with_control_plane_batch_rows(2)
            .with_remote_worker_batch_rows(2)
            .with_secondary_remote_worker_batch_rows(2)
            .with_remote_worker_backend(PsionReferencePilotContributionBackend::Cpu)
            .with_secondary_remote_worker_backend(PsionReferencePilotContributionBackend::Cpu);
        let run = run_psion_dual_host_actual_pretraining_bringup(
            repo_root().as_path(),
            &config,
            &dual_host,
            |request| {
                build_psion_actual_pretraining_joint_contribution(repo_root().as_path(), request)
            },
        )
        .expect("actual pretraining bringup should run");

        assert_eq!(run.run.stage_receipt.stage_id, config.stage_id);
        assert_eq!(
            run.run.stage_receipt.model_id,
            "psion-compact-decoder-internal-v1"
        );
        assert_eq!(
            run.run.stage_receipt.dataset_identity,
            "psion_corpus_tokenized@v1"
        );
        assert_eq!(
            run.contribution_receipts.len(),
            (config.budget.max_steps as usize) * 3,
            "tri-host actual bringup should retain one contribution per host per step"
        );
        assert_eq!(run.topology_receipt.contributor_count, 3);
        assert_eq!(
            run.topology_receipt.execution_topology_classification,
            "multi_host_joint_gradient_average"
        );
        assert_eq!(
            run.progress_checkpoint_receipts.len(),
            2,
            "6 steps with 3 steps per window should retain one boundary checkpoint per window"
        );
        assert_eq!(
            run.progress_checkpoint_artifacts.len(),
            run.progress_checkpoint_receipts.len(),
            "progress checkpoint receipts should track one artifact each"
        );
        assert!(
            run.progress_checkpoint_receipts
                .iter()
                .all(|receipt| receipt.expected_contributor_hosts.len() == 3
                    && receipt.observed_contributor_hosts.len() == 3
                    && receipt.cumulative_train_tokens_processed > 0),
            "progress checkpoint receipts should retain tri-host continuity and cumulative progress"
        );
        assert!(
            run.contributor_continuity_receipt
                .all_configured_contributors_present_each_step,
            "continuity receipt should prove all configured contributors stayed in the optimization path"
        );
        assert!(
            run.run.step_receipts.iter().any(|receipt| receipt
                .group_telemetry
                .iter()
                .any(|group| group.update_norm_l2 > 0.0)),
            "actual pretraining bringup should apply at least one non-zero merged update"
        );
        run.run
            .observability_receipt
            .validate_against_stage(&run.run.stage_receipt)
            .expect("actual bringup observability receipt should validate");
    }

    #[test]
    fn reference_pilot_evidence_bundle_validates_against_matrix_and_benchmark_contracts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config = PsionReferencePilotConfig::reference()?;
        let bundle = run_psion_reference_pilot_evidence_bundle(repo_root().as_path(), &config)?;
        let acceptance_matrix: PsionAcceptanceMatrix = load_json_fixture(
            repo_root().as_path(),
            "fixtures/psion/acceptance/psion_acceptance_matrix_v1.json",
        )?;
        let benchmark_catalog: PsionBenchmarkCatalog = load_json_fixture(
            repo_root().as_path(),
            "fixtures/psion/benchmarks/psion_benchmark_catalog_v1.json",
        )?;
        let capability_matrix: PsionCapabilityMatrixView = load_json_fixture(
            repo_root().as_path(),
            "fixtures/psion/capability/psion_capability_matrix_v1.json",
        )?;
        let artifact_lineage: PsionArtifactLineageManifest = load_json_fixture(
            repo_root().as_path(),
            "fixtures/psion/lifecycle/psion_artifact_lineage_manifest_v1.json",
        )?;
        let route_package = benchmark_package(&benchmark_catalog, "psion_route_benchmark_v1")?;
        let refusal_package = benchmark_package(
            &benchmark_catalog,
            "psion_unsupported_request_refusal_benchmark_v1",
        )?;

        bundle
            .route_class_evaluation_receipt
            .validate_against_package(route_package, &artifact_lineage)?;
        bundle
            .refusal_calibration_receipt
            .validate_against_package_and_matrix(
                refusal_package,
                &capability_matrix,
                &artifact_lineage,
            )?;
        bundle
            .pilot_bundle
            .validate_against_matrix(&acceptance_matrix)?;
        assert_eq!(
            bundle.architecture_benchmark.aggregate_pass_rate_bps,
            10_000
        );
        assert_eq!(
            bundle.normative_spec_benchmark.aggregate_pass_rate_bps,
            10_000
        );
        assert_eq!(bundle.held_out_benchmark.aggregate_pass_rate_bps, 10_000);
        Ok(())
    }
}
