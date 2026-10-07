use std::collections::BTreeSet;

use psionic_collectives::CollectiveSyncScope;
use psionic_runtime::{
    ClusterCommunicationClass, ClusterTransportClass, ExecutionTopologyKind,
    TrainingCheckpointAvailability, TrainingCollectiveKind, TrainingCollectiveQuantization,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    DistributedOptimizerContract, DistributedOptimizerStepReceipt, PsionCheckpointLayoutKind,
    PsionCheckpointRecoveryBundle, PsionCheckpointRecoveryContractError,
    PsionCheckpointRecoveryEventKind, PsionPretrainRunObservabilityError,
    PsionPretrainRunObservabilityReceipt, PsionPretrainRunScaleProfile,
    PsionPretrainStageRunReceipt, PsionRentedClusterRunbook, ReplayVerificationDisposition,
    TrainingDistributedOptimizerKind, TrainingRecoveryMode, TrainingReplayReceipt,
    TrainingReplayVerificationReceipt,
};

/// Stable schema version for the Psion trusted-cluster topology contract.
pub const PSION_TRUSTED_CLUSTER_TOPOLOGY_CONTRACT_SCHEMA_VERSION: &str =
    "psion.trusted_cluster_topology_contract.v1";
/// Stable schema version for the Psion trusted-cluster replay receipt.
pub const PSION_TRUSTED_CLUSTER_REPLAY_RECEIPT_SCHEMA_VERSION: &str =
    "psion.trusted_cluster_replay_receipt.v1";
/// Stable schema version for the Psion trusted-cluster run bundle.
pub const PSION_TRUSTED_CLUSTER_RUN_BUNDLE_SCHEMA_VERSION: &str =
    "psion.trusted_cluster_run_bundle.v1";
/// Minimum replay count frozen for the first trusted-cluster receipt.
pub const PSION_TRUSTED_CLUSTER_MINIMUM_SUCCESSFUL_REPLAYS: u16 = 5;

/// Supported or explicitly refused topology mode for the first trusted-cluster lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionTrustedClusterTopologyMode {
    /// Homogeneous H100-class CUDA workers with one rank per node on trusted-LAN stream transport.
    HomogeneousCudaH100OneRankPerNode,
    /// Mixed backend or accelerator families in one mesh.
    MixedBackendOrAccelerator,
    /// Cross-region or otherwise mixed-latency cluster fabric.
    CrossRegionOrMixedLatencyFabric,
    /// Elastic world-size expansion beyond the current bounded lane.
    ElasticWorldSizeChange,
    /// Shared or otherwise untrusted cluster fabric.
    SharedOrUntrustedFabric,
}

/// Final disposition for one trusted-cluster topology mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionTrustedClusterTopologyDisposition {
    /// Mode is explicitly supported by the first trusted-cluster lane.
    Supported,
    /// Mode is explicitly refused.
    Refused,
    /// Mode is deliberately kept outside the first truthful lane.
    OutOfScope,
}

/// One explicit trusted-cluster topology evaluation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTrustedClusterModeEvaluation {
    /// Topology mode being evaluated.
    pub topology_mode: PsionTrustedClusterTopologyMode,
    /// Final support or refusal posture.
    pub disposition: PsionTrustedClusterTopologyDisposition,
    /// Short detail explaining the outcome.
    pub detail: String,
}

/// Public distributed backend family frozen by the trusted-cluster lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionTrustedClusterDistributedBackendFamily {
    /// NCCL-class CUDA distributed group truth.
    Nccl,
}

/// Public distributed topology profile frozen by the trusted-cluster lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionTrustedClusterDistributedTopologyProfile {
    /// Peer ranks communicate over trusted-LAN stream transport.
    TrustedLanStreamMesh,
}

/// Public distributed group provenance frozen by the trusted-cluster lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PsionTrustedClusterDistributedGroupKind {
    /// The group comes from explicit mesh/bootstrap truth.
    BootstrappedMesh,
}

/// Train-owned distributed-group receipt for the trusted-cluster lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTrustedClusterDistributedGroupReceipt {
    /// Stable group identifier.
    pub group_id: String,
    /// Provenance of the distributed group.
    pub kind: PsionTrustedClusterDistributedGroupKind,
    /// Requested public distributed backend family.
    pub requested_backend: PsionTrustedClusterDistributedBackendFamily,
    /// Resolved public distributed backend family.
    pub resolved_backend: PsionTrustedClusterDistributedBackendFamily,
    /// Effective runtime backend carried by the underlying mesh.
    pub effective_backend: String,
    /// Communication class carried by the underlying mesh.
    pub communication_class: ClusterCommunicationClass,
    /// Transport class carried by the underlying mesh.
    pub transport: ClusterTransportClass,
    /// Bounded public topology profile for the group.
    pub topology_profile: PsionTrustedClusterDistributedTopologyProfile,
    /// Stable mesh identifier.
    pub mesh_id: String,
    /// Monotonic mesh revision.
    pub mesh_revision: u64,
    /// Stable local node identifier inside the group.
    pub local_node_id: String,
    /// Local rank inside the group.
    pub local_rank: usize,
    /// Total world size.
    pub world_size: usize,
    /// Ordered member node ids in the group.
    pub member_node_ids: Vec<String>,
    /// Short summary of the distributed-group posture.
    pub summary: String,
    /// Stable digest over the group receipt.
    pub group_digest: String,
}

impl PsionTrustedClusterDistributedGroupReceipt {
    fn validate(&self) -> Result<(), PsionTrustedClusterRunError> {
        ensure_nonempty(
            self.group_id.as_str(),
            "trusted_cluster_distributed_group.group_id",
        )?;
        ensure_nonempty(
            self.effective_backend.as_str(),
            "trusted_cluster_distributed_group.effective_backend",
        )?;
        ensure_nonempty(
            self.mesh_id.as_str(),
            "trusted_cluster_distributed_group.mesh_id",
        )?;
        ensure_nonempty(
            self.local_node_id.as_str(),
            "trusted_cluster_distributed_group.local_node_id",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "trusted_cluster_distributed_group.summary",
        )?;
        if self.world_size < 2 {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_distributed_group.world_size"),
                expected: String::from("at least 2"),
                actual: self.world_size.to_string(),
            });
        }
        if self.member_node_ids.len() != self.world_size {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_distributed_group.member_node_ids"),
                expected: self.world_size.to_string(),
                actual: self.member_node_ids.len().to_string(),
            });
        }
        if !self
            .member_node_ids
            .iter()
            .any(|node_id| node_id == &self.local_node_id)
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_distributed_group.local_node_id"),
                expected: serde_json::to_string(&self.member_node_ids).unwrap_or_default(),
                actual: self.local_node_id.clone(),
            });
        }
        if self.local_rank >= self.world_size {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_distributed_group.local_rank"),
                expected: format!("rank < {}", self.world_size),
                actual: self.local_rank.to_string(),
            });
        }
        if self.group_digest != stable_trusted_cluster_distributed_group_digest(self) {
            return Err(PsionTrustedClusterRunError::ReceiptDigestMismatch {
                receipt_kind: String::from("trusted_cluster_distributed_group"),
            });
        }
        Ok(())
    }
}

/// Bounded topology contract for the first trusted-cluster Psion lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PsionTrustedClusterTopologyContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable contract identifier.
    pub contract_id: String,
    /// Bound broader-stage receipt digest.
    pub pretrain_stage_receipt_digest: String,
    /// Bound broader-run observability receipt digest.
    pub observability_receipt_digest: String,
    /// Bound rented-cluster runbook digest preserving the narrower refusal boundary.
    pub rented_cluster_runbook_digest: String,
    /// Short supported-topology label.
    pub supported_topology_label: String,
    /// Required runtime backend for this lane.
    pub required_backend: String,
    /// Required stable device-id prefix for selected accelerators.
    pub required_device_id_prefix: String,
    /// Worker count required by the supported topology.
    pub required_worker_count: u16,
    /// Whether exactly one rank per node is required.
    pub one_rank_per_node_required: bool,
    /// Required public distributed backend family for this lane.
    pub required_distributed_backend: PsionTrustedClusterDistributedBackendFamily,
    /// Required public distributed topology profile for this lane.
    pub required_distributed_topology_profile: PsionTrustedClusterDistributedTopologyProfile,
    /// Required trusted-cluster transport class.
    pub required_transport: ClusterTransportClass,
    /// Required cluster disposition for the realized execution path.
    pub required_cluster_disposition: psionic_runtime::ClusterExecutionDisposition,
    /// Required execution-topology kind for the realized path.
    pub required_execution_topology_kind: ExecutionTopologyKind,
    /// Required mesh communication class.
    pub required_communication_class: ClusterCommunicationClass,
    /// Required distributed-optimizer family.
    pub required_optimizer_kind: TrainingDistributedOptimizerKind,
    /// Required collective kind.
    pub required_collective_kind: TrainingCollectiveKind,
    /// Required collective quantization mode.
    pub required_collective_quantization: TrainingCollectiveQuantization,
    /// Explicit supported and refused mode evaluations.
    pub mode_evaluations: Vec<PsionTrustedClusterModeEvaluation>,
    /// Short summary of the bounded lane.
    pub summary: String,
    /// Stable digest over the contract.
    pub contract_digest: String,
}

impl PsionTrustedClusterTopologyContract {
    /// Validates the trusted-cluster topology contract against the realized clustered inputs.
    pub fn validate_against_inputs(
        &self,
        stage_receipt: &PsionPretrainStageRunReceipt,
        observability_receipt: &PsionPretrainRunObservabilityReceipt,
        rented_cluster_runbook: &PsionRentedClusterRunbook,
        distributed_group: &PsionTrustedClusterDistributedGroupReceipt,
        optimizer_contract: &DistributedOptimizerContract,
    ) -> Result<(), PsionTrustedClusterRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "trusted_cluster_topology_contract.schema_version",
        )?;
        if self.schema_version != PSION_TRUSTED_CLUSTER_TOPOLOGY_CONTRACT_SCHEMA_VERSION {
            return Err(PsionTrustedClusterRunError::SchemaVersionMismatch {
                expected: String::from(PSION_TRUSTED_CLUSTER_TOPOLOGY_CONTRACT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.contract_id.as_str(),
            "trusted_cluster_topology_contract.contract_id",
        )?;
        check_string_match(
            self.pretrain_stage_receipt_digest.as_str(),
            stage_receipt.receipt_digest.as_str(),
            "trusted_cluster_topology_contract.pretrain_stage_receipt_digest",
        )?;
        check_string_match(
            self.observability_receipt_digest.as_str(),
            observability_receipt.observability_digest.as_str(),
            "trusted_cluster_topology_contract.observability_receipt_digest",
        )?;
        check_string_match(
            self.rented_cluster_runbook_digest.as_str(),
            rented_cluster_runbook.runbook_digest.as_str(),
            "trusted_cluster_topology_contract.rented_cluster_runbook_digest",
        )?;
        ensure_nonempty(
            self.supported_topology_label.as_str(),
            "trusted_cluster_topology_contract.supported_topology_label",
        )?;
        ensure_nonempty(
            self.required_backend.as_str(),
            "trusted_cluster_topology_contract.required_backend",
        )?;
        ensure_nonempty(
            self.required_device_id_prefix.as_str(),
            "trusted_cluster_topology_contract.required_device_id_prefix",
        )?;
        ensure_nonempty(
            self.summary.as_str(),
            "trusted_cluster_topology_contract.summary",
        )?;
        if self.required_worker_count < 2 {
            return Err(PsionTrustedClusterRunError::InvalidWorkerCount {
                actual: self.required_worker_count,
            });
        }
        self.validate_mode_evaluations()?;

        if observability_receipt.run_profile != PsionPretrainRunScaleProfile::BroaderPretraining {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_topology_contract.run_profile"),
                expected: String::from("broader_pretraining"),
                actual: format!("{:?}", observability_receipt.run_profile),
            });
        }
        if observability_receipt
            .hardware_topology
            .observed_worker_count
            != self.required_worker_count
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_topology_contract.observed_worker_count"),
                expected: self.required_worker_count.to_string(),
                actual: observability_receipt
                    .hardware_topology
                    .observed_worker_count
                    .to_string(),
            });
        }

        let delivered_execution = &observability_receipt.hardware_topology.delivered_execution;
        check_string_match(
            delivered_execution.runtime_backend.as_str(),
            self.required_backend.as_str(),
            "trusted_cluster_topology_contract.delivered_execution.runtime_backend",
        )?;
        if delivered_execution.selected_devices.len() != usize::from(self.required_worker_count) {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.delivered_execution.selected_devices",
                ),
                expected: self.required_worker_count.to_string(),
                actual: delivered_execution.selected_devices.len().to_string(),
            });
        }
        for device in &delivered_execution.selected_devices {
            ensure_device_prefix(
                device.stable_device_id.as_str(),
                self.required_device_id_prefix.as_str(),
            )?;
        }
        let execution_topology = delivered_execution
            .execution_topology
            .as_ref()
            .ok_or(PsionTrustedClusterRunError::MissingExecutionTopology)?;
        if execution_topology.kind != self.required_execution_topology_kind {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_topology_contract.execution_topology.kind"),
                expected: format!("{:?}", self.required_execution_topology_kind),
                actual: format!("{:?}", execution_topology.kind),
            });
        }
        if execution_topology.assignments.len() != usize::from(self.required_worker_count) {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.execution_topology.assignments",
                ),
                expected: self.required_worker_count.to_string(),
                actual: execution_topology.assignments.len().to_string(),
            });
        }

        let cluster_execution = delivered_execution
            .cluster_execution
            .as_ref()
            .ok_or(PsionTrustedClusterRunError::MissingClusterExecution)?;
        if cluster_execution.disposition != self.required_cluster_disposition {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.cluster_execution.disposition",
                ),
                expected: format!("{:?}", self.required_cluster_disposition),
                actual: format!("{:?}", cluster_execution.disposition),
            });
        }
        if cluster_execution.transport != self.required_transport {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.cluster_execution.transport",
                ),
                expected: format!("{:?}", self.required_transport),
                actual: format!("{:?}", cluster_execution.transport),
            });
        }
        let selected_node_ids = cluster_execution
            .selected_nodes
            .iter()
            .map(|node| node.node_id.clone())
            .collect::<Vec<_>>();
        if selected_node_ids.len() != usize::from(self.required_worker_count) {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.cluster_execution.selected_nodes",
                ),
                expected: self.required_worker_count.to_string(),
                actual: selected_node_ids.len().to_string(),
            });
        }
        if self.one_rank_per_node_required
            && distinct_count(selected_node_ids.as_slice())
                != usize::from(self.required_worker_count)
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.cluster_execution.selected_nodes",
                ),
                expected: format!("{} unique node ids", self.required_worker_count),
                actual: format!(
                    "{} unique node ids",
                    distinct_count(selected_node_ids.as_slice())
                ),
            });
        }
        for node in &cluster_execution.selected_nodes {
            check_string_match(
                node.runtime_backend.as_str(),
                self.required_backend.as_str(),
                "trusted_cluster_topology_contract.cluster_execution.selected_nodes[].runtime_backend",
            )?;
            let device_inventory = node.device_inventory.as_ref().ok_or(
                PsionTrustedClusterRunError::MissingField {
                    field: String::from(
                        "trusted_cluster_topology_contract.cluster_execution.selected_nodes[].device_inventory",
                    ),
                },
            )?;
            ensure_device_prefix(
                device_inventory.stable_device_id.as_str(),
                self.required_device_id_prefix.as_str(),
            )?;
        }
        check_string_match(
            cluster_execution.topology_digest.as_str(),
            stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .topology_digest
                .as_str(),
            "trusted_cluster_topology_contract.cluster_execution.topology_digest",
        )?;

        let training_collective = cluster_execution
            .training_collective
            .as_ref()
            .ok_or(PsionTrustedClusterRunError::MissingTrainingCollective)?;
        if training_collective.kind != self.required_collective_kind {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_topology_contract.training_collective.kind"),
                expected: format!("{:?}", self.required_collective_kind),
                actual: format!("{:?}", training_collective.kind),
            });
        }
        if training_collective.quantization != self.required_collective_quantization {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.training_collective.quantization",
                ),
                expected: format!("{:?}", self.required_collective_quantization),
                actual: format!("{:?}", training_collective.quantization),
            });
        }
        if training_collective.worker_count != usize::from(self.required_worker_count) {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.training_collective.worker_count",
                ),
                expected: self.required_worker_count.to_string(),
                actual: training_collective.worker_count.to_string(),
            });
        }
        check_string_match(
            training_collective.device_mesh.effective_backend.as_str(),
            self.required_backend.as_str(),
            "trusted_cluster_topology_contract.training_collective.device_mesh.effective_backend",
        )?;
        if training_collective.device_mesh.communication_class != self.required_communication_class
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.training_collective.device_mesh.communication_class",
                ),
                expected: format!("{:?}", self.required_communication_class),
                actual: format!("{:?}", training_collective.device_mesh.communication_class),
            });
        }
        compare_string_sets(
            training_collective.device_mesh.member_node_ids.as_slice(),
            selected_node_ids.as_slice(),
            "trusted_cluster_topology_contract.training_collective.device_mesh.member_node_ids",
        )?;

        let training_recovery = cluster_execution
            .training_recovery
            .as_ref()
            .ok_or(PsionTrustedClusterRunError::MissingTrainingRecovery)?;
        if training_recovery.checkpoint_availability != TrainingCheckpointAvailability::Durable {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.training_recovery.checkpoint_availability",
                ),
                expected: String::from("durable"),
                actual: format!("{:?}", training_recovery.checkpoint_availability),
            });
        }
        let latest_checkpoint = training_recovery.latest_checkpoint.as_ref().ok_or(
            PsionTrustedClusterRunError::MissingField {
                field: String::from(
                    "trusted_cluster_topology_contract.training_recovery.latest_checkpoint",
                ),
            },
        )?;
        check_string_match(
            latest_checkpoint.topology_digest.as_str(),
            cluster_execution.topology_digest.as_str(),
            "trusted_cluster_topology_contract.training_recovery.latest_checkpoint.topology_digest",
        )?;
        compare_string_sets(
            training_recovery
                .elastic_membership
                .active_node_ids
                .as_slice(),
            selected_node_ids.as_slice(),
            "trusted_cluster_topology_contract.training_recovery.elastic_membership.active_node_ids",
        )?;

        distributed_group.validate()?;
        if distributed_group.kind != PsionTrustedClusterDistributedGroupKind::BootstrappedMesh {
            return Err(
                PsionTrustedClusterRunError::UnsupportedDistributedGroupKind {
                    actual: distributed_group.kind,
                },
            );
        }
        if distributed_group.world_size != usize::from(self.required_worker_count) {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_topology_contract.distributed_group.size"),
                expected: self.required_worker_count.to_string(),
                actual: distributed_group.world_size.to_string(),
            });
        }
        check_string_match(
            distributed_group.effective_backend.as_str(),
            self.required_backend.as_str(),
            "trusted_cluster_topology_contract.distributed_group.effective_backend",
        )?;
        if distributed_group.transport != self.required_transport {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.distributed_group.transport",
                ),
                expected: format!("{:?}", self.required_transport),
                actual: format!("{:?}", distributed_group.transport),
            });
        }
        if distributed_group.communication_class != self.required_communication_class {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.distributed_group.communication_class",
                ),
                expected: format!("{:?}", self.required_communication_class),
                actual: format!("{:?}", distributed_group.communication_class),
            });
        }
        if distributed_group.resolved_backend != self.required_distributed_backend {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.distributed_group.resolved_backend",
                ),
                expected: format!("{:?}", self.required_distributed_backend),
                actual: format!("{:?}", distributed_group.resolved_backend),
            });
        }
        if distributed_group.topology_profile != self.required_distributed_topology_profile {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.distributed_group.topology_profile",
                ),
                expected: format!("{:?}", self.required_distributed_topology_profile),
                actual: format!("{:?}", distributed_group.topology_profile),
            });
        }
        check_string_match(
            distributed_group.mesh_id.as_str(),
            training_collective.device_mesh.mesh_id.as_str(),
            "trusted_cluster_topology_contract.distributed_group.mesh_id",
        )?;
        if distributed_group.mesh_revision != training_collective.device_mesh.mesh_revision {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.distributed_group.mesh_revision",
                ),
                expected: training_collective.device_mesh.mesh_revision.to_string(),
                actual: distributed_group.mesh_revision.to_string(),
            });
        }
        compare_string_sets(
            distributed_group.member_node_ids.as_slice(),
            selected_node_ids.as_slice(),
            "trusted_cluster_topology_contract.distributed_group.members",
        )?;

        if optimizer_contract.optimizer_kind != self.required_optimizer_kind {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_topology_contract.optimizer_contract.optimizer_kind",
                ),
                expected: format!("{:?}", self.required_optimizer_kind),
                actual: format!("{:?}", optimizer_contract.optimizer_kind),
            });
        }
        validate_optimizer_plan_alignment(
            optimizer_contract,
            distributed_group,
            training_collective,
            self.required_communication_class,
            self.required_collective_kind,
            self.required_collective_quantization,
        )?;

        if self.contract_digest != stable_trusted_cluster_topology_contract_digest(self) {
            return Err(PsionTrustedClusterRunError::ContractDigestMismatch);
        }
        Ok(())
    }

    fn validate_mode_evaluations(&self) -> Result<(), PsionTrustedClusterRunError> {
        if self.mode_evaluations.is_empty() {
            return Err(PsionTrustedClusterRunError::MissingField {
                field: String::from("trusted_cluster_topology_contract.mode_evaluations"),
            });
        }
        let mut seen_modes = BTreeSet::new();
        let mut supported_lane_present = false;
        let mut refused_or_out_of_scope_present = false;
        for evaluation in &self.mode_evaluations {
            if !seen_modes.insert(evaluation.topology_mode) {
                return Err(PsionTrustedClusterRunError::DuplicateTopologyMode {
                    topology_mode: trusted_cluster_topology_mode_name(evaluation.topology_mode)
                        .to_owned(),
                });
            }
            ensure_nonempty(
                evaluation.detail.as_str(),
                "trusted_cluster_topology_contract.mode_evaluations[].detail",
            )?;
            match evaluation.disposition {
                PsionTrustedClusterTopologyDisposition::Supported => {
                    if evaluation.topology_mode
                        != PsionTrustedClusterTopologyMode::HomogeneousCudaH100OneRankPerNode
                    {
                        return Err(PsionTrustedClusterRunError::FieldMismatch {
                            field: String::from(
                                "trusted_cluster_topology_contract.mode_evaluations[].disposition",
                            ),
                            expected: String::from(
                                "supported only for homogeneous_cuda_h100_one_rank_per_node",
                            ),
                            actual: trusted_cluster_topology_mode_name(evaluation.topology_mode)
                                .to_owned(),
                        });
                    }
                    supported_lane_present = true;
                }
                PsionTrustedClusterTopologyDisposition::Refused
                | PsionTrustedClusterTopologyDisposition::OutOfScope => {
                    refused_or_out_of_scope_present = true;
                }
            }
        }
        if !supported_lane_present || !refused_or_out_of_scope_present {
            return Err(PsionTrustedClusterRunError::TopologyCoverageMissing {
                supported_lane_present,
                refused_or_out_of_scope_present,
            });
        }
        Ok(())
    }
}

/// Multi-host replay receipt for the first trusted-cluster Psion lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionTrustedClusterReplayReceipt {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable receipt identifier.
    pub receipt_id: String,
    /// Bound trusted-cluster topology-contract digest.
    pub topology_contract_digest: String,
    /// Ordered participating node ids covered by the replay exercise.
    pub participating_node_ids: Vec<String>,
    /// Baseline multi-host replay receipt.
    pub baseline_replay: TrainingReplayReceipt,
    /// Observed multi-host replay receipt.
    pub observed_replay: TrainingReplayReceipt,
    /// Verification receipt comparing the two replay payloads.
    pub verification: TrainingReplayVerificationReceipt,
    /// Number of successful replay checks completed for the lane.
    pub successful_replays: u16,
    /// Whether exact replay was observed.
    pub exact_replay_observed: bool,
    /// Short summary of the replay surface.
    pub summary: String,
    /// Stable digest over the replay receipt.
    pub receipt_digest: String,
}

impl PsionTrustedClusterReplayReceipt {
    /// Validates the trusted-cluster replay receipt against the topology contract and group truth.
    pub fn validate_against_inputs(
        &self,
        topology_contract: &PsionTrustedClusterTopologyContract,
        distributed_group: &PsionTrustedClusterDistributedGroupReceipt,
    ) -> Result<(), PsionTrustedClusterRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "trusted_cluster_replay_receipt.schema_version",
        )?;
        if self.schema_version != PSION_TRUSTED_CLUSTER_REPLAY_RECEIPT_SCHEMA_VERSION {
            return Err(PsionTrustedClusterRunError::SchemaVersionMismatch {
                expected: String::from(PSION_TRUSTED_CLUSTER_REPLAY_RECEIPT_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.receipt_id.as_str(),
            "trusted_cluster_replay_receipt.receipt_id",
        )?;
        check_string_match(
            self.topology_contract_digest.as_str(),
            topology_contract.contract_digest.as_str(),
            "trusted_cluster_replay_receipt.topology_contract_digest",
        )?;
        if self.successful_replays < PSION_TRUSTED_CLUSTER_MINIMUM_SUCCESSFUL_REPLAYS {
            return Err(PsionTrustedClusterRunError::ReplayCoverageTooSmall {
                expected_minimum: PSION_TRUSTED_CLUSTER_MINIMUM_SUCCESSFUL_REPLAYS,
                actual: self.successful_replays,
            });
        }
        if !self.exact_replay_observed {
            return Err(PsionTrustedClusterRunError::ReplayVerificationDrifted);
        }
        ensure_nonempty(
            self.summary.as_str(),
            "trusted_cluster_replay_receipt.summary",
        )?;

        let recomputed_verification = self.baseline_replay.verify_against(&self.observed_replay);
        if recomputed_verification != self.verification {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_replay_receipt.verification"),
                expected: serde_json::to_string(&recomputed_verification).unwrap_or_default(),
                actual: serde_json::to_string(&self.verification).unwrap_or_default(),
            });
        }
        if self.verification.disposition != ReplayVerificationDisposition::Match {
            return Err(PsionTrustedClusterRunError::ReplayVerificationDrifted);
        }
        check_string_match(
            self.verification.expected_replay_digest.as_str(),
            self.baseline_replay.replay_digest.as_str(),
            "trusted_cluster_replay_receipt.verification.expected_replay_digest",
        )?;
        check_string_match(
            self.verification.observed_replay_digest.as_str(),
            self.observed_replay.replay_digest.as_str(),
            "trusted_cluster_replay_receipt.verification.observed_replay_digest",
        )?;

        let expected_nodes = distributed_group.member_node_ids.clone();
        compare_string_sets(
            self.participating_node_ids.as_slice(),
            expected_nodes.as_slice(),
            "trusted_cluster_replay_receipt.participating_node_ids",
        )?;
        if self.participating_node_ids.len() != usize::from(topology_contract.required_worker_count)
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_replay_receipt.participating_node_ids"),
                expected: topology_contract.required_worker_count.to_string(),
                actual: self.participating_node_ids.len().to_string(),
            });
        }
        compare_string_sets(
            baseline_replay_worker_ids(&self.baseline_replay)?.as_slice(),
            self.participating_node_ids.as_slice(),
            "trusted_cluster_replay_receipt.baseline_replay.sample_selection_rules",
        )?;
        compare_string_sets(
            baseline_replay_worker_ids(&self.observed_replay)?.as_slice(),
            self.participating_node_ids.as_slice(),
            "trusted_cluster_replay_receipt.observed_replay.sample_selection_rules",
        )?;

        if self.receipt_digest != stable_trusted_cluster_replay_receipt_digest(self) {
            return Err(PsionTrustedClusterRunError::ReceiptDigestMismatch {
                receipt_kind: String::from("trusted_cluster_replay_receipt"),
            });
        }
        Ok(())
    }
}

/// Self-contained trusted-cluster run bundle for the first multi-host Psion lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PsionTrustedClusterRunBundle {
    /// Stable schema version.
    pub schema_version: String,
    /// Stable bundle identifier.
    pub bundle_id: String,
    /// Bound broader pretrain-stage receipt.
    pub pretrain_stage_receipt: PsionPretrainStageRunReceipt,
    /// Bound broader-run observability receipt.
    pub observability_receipt: PsionPretrainRunObservabilityReceipt,
    /// Bound rented-cluster runbook proving the trusted-cluster lane does not widen rented scope.
    pub rented_cluster_runbook: PsionRentedClusterRunbook,
    /// Trusted-cluster topology contract.
    pub topology_contract: PsionTrustedClusterTopologyContract,
    /// Public distributed-group snapshot aligned with the cluster mesh.
    pub distributed_group: PsionTrustedClusterDistributedGroupReceipt,
    /// Distributed-optimizer contract for the run.
    pub optimizer_contract: DistributedOptimizerContract,
    /// One realized distributed step receipt.
    pub distributed_step_receipt: DistributedOptimizerStepReceipt,
    /// Checkpoint recovery bundle generated on the trusted-cluster run path.
    pub checkpoint_recovery_bundle: PsionCheckpointRecoveryBundle,
    /// Multi-host replay receipt for the trusted-cluster lane.
    pub replay_receipt: PsionTrustedClusterReplayReceipt,
    /// Short operator-visible summary.
    pub summary: String,
    /// Stable digest over the bundle.
    pub bundle_digest: String,
}

impl PsionTrustedClusterRunBundle {
    /// Validates the trusted-cluster run bundle against its bound receipts and contracts.
    pub fn validate(&self) -> Result<(), PsionTrustedClusterRunError> {
        ensure_nonempty(
            self.schema_version.as_str(),
            "trusted_cluster_run_bundle.schema_version",
        )?;
        if self.schema_version != PSION_TRUSTED_CLUSTER_RUN_BUNDLE_SCHEMA_VERSION {
            return Err(PsionTrustedClusterRunError::SchemaVersionMismatch {
                expected: String::from(PSION_TRUSTED_CLUSTER_RUN_BUNDLE_SCHEMA_VERSION),
                actual: self.schema_version.clone(),
            });
        }
        ensure_nonempty(
            self.bundle_id.as_str(),
            "trusted_cluster_run_bundle.bundle_id",
        )?;
        ensure_nonempty(self.summary.as_str(), "trusted_cluster_run_bundle.summary")?;

        self.observability_receipt
            .validate_against_stage(&self.pretrain_stage_receipt)
            .map_err(PsionTrustedClusterRunError::ObservabilityContract)?;
        self.checkpoint_recovery_bundle
            .validate_against_inputs(&self.pretrain_stage_receipt, &self.observability_receipt)
            .map_err(PsionTrustedClusterRunError::CheckpointRecoveryContract)?;
        self.topology_contract.validate_against_inputs(
            &self.pretrain_stage_receipt,
            &self.observability_receipt,
            &self.rented_cluster_runbook,
            &self.distributed_group,
            &self.optimizer_contract,
        )?;
        self.validate_distributed_step_receipt()?;
        self.validate_checkpoint_restart_coverage()?;
        self.replay_receipt
            .validate_against_inputs(&self.topology_contract, &self.distributed_group)?;

        if self.bundle_digest != stable_trusted_cluster_run_bundle_digest(self) {
            return Err(PsionTrustedClusterRunError::ReceiptDigestMismatch {
                receipt_kind: String::from("trusted_cluster_run_bundle"),
            });
        }
        Ok(())
    }

    fn validate_distributed_step_receipt(&self) -> Result<(), PsionTrustedClusterRunError> {
        check_string_match(
            self.distributed_step_receipt.contract_digest.as_str(),
            self.optimizer_contract.contract_digest.as_str(),
            "trusted_cluster_run_bundle.distributed_step_receipt.contract_digest",
        )?;
        check_string_match(
            self.distributed_step_receipt
                .memory_plan
                .contract_digest
                .as_str(),
            self.optimizer_contract.contract_digest.as_str(),
            "trusted_cluster_run_bundle.distributed_step_receipt.memory_plan.contract_digest",
        )?;
        if self.distributed_step_receipt.precision_policy
            != self.optimizer_contract.precision_policy
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.distributed_step_receipt.precision_policy",
                ),
                expected: serde_json::to_string(&self.optimizer_contract.precision_policy)
                    .unwrap_or_default(),
                actual: serde_json::to_string(&self.distributed_step_receipt.precision_policy)
                    .unwrap_or_default(),
            });
        }
        if self.distributed_step_receipt.collective_sync_plan
            != self.optimizer_contract.collective_sync_plan
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.distributed_step_receipt.collective_sync_plan",
                ),
                expected: serde_json::to_string(&self.optimizer_contract.collective_sync_plan)
                    .unwrap_or_default(),
                actual: serde_json::to_string(&self.distributed_step_receipt.collective_sync_plan)
                    .unwrap_or_default(),
            });
        }
        if self.distributed_step_receipt.groups.len() != self.optimizer_contract.groups.len() {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from("trusted_cluster_run_bundle.distributed_step_receipt.groups"),
                expected: self.optimizer_contract.groups.len().to_string(),
                actual: self.distributed_step_receipt.groups.len().to_string(),
            });
        }
        if self.distributed_step_receipt.microbatches.is_empty() {
            return Err(PsionTrustedClusterRunError::MissingField {
                field: String::from(
                    "trusted_cluster_run_bundle.distributed_step_receipt.microbatches",
                ),
            });
        }
        if self
            .distributed_step_receipt
            .collective_sync_plan
            .cadence_receipt
            .mesh_revision
            != self.distributed_group.mesh_revision
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.distributed_step_receipt.collective_sync_plan.cadence_receipt.mesh_revision",
                ),
                expected: self.distributed_group.mesh_revision.to_string(),
                actual: self
                    .distributed_step_receipt
                    .collective_sync_plan
                    .cadence_receipt
                    .mesh_revision
                    .to_string(),
            });
        }
        Ok(())
    }

    fn validate_checkpoint_restart_coverage(&self) -> Result<(), PsionTrustedClusterRunError> {
        let restart_event = self
            .checkpoint_recovery_bundle
            .recovery_events
            .iter()
            .find(|event| event.event_kind == PsionCheckpointRecoveryEventKind::DistributedRestart)
            .ok_or(PsionTrustedClusterRunError::MissingDistributedRestartCoverage)?;
        let source_artifact = self
            .checkpoint_recovery_bundle
            .checkpoint_artifacts
            .iter()
            .find(|artifact| artifact.artifact_id == restart_event.source_artifact_id)
            .ok_or(PsionTrustedClusterRunError::MissingDistributedRestartCoverage)?;
        if source_artifact.layout_kind != PsionCheckpointLayoutKind::Sharded {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].source_artifact.layout_kind",
                ),
                expected: String::from("sharded"),
                actual: format!("{:?}", source_artifact.layout_kind),
            });
        }
        if restart_event.recovery_mode != TrainingRecoveryMode::BlockingCatchUp {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].recovery_mode",
                ),
                expected: String::from("blocking_catch_up"),
                actual: format!("{:?}", restart_event.recovery_mode),
            });
        }
        if restart_event.recovered_worker_count != self.topology_contract.required_worker_count {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].recovered_worker_count",
                ),
                expected: self.topology_contract.required_worker_count.to_string(),
                actual: restart_event.recovered_worker_count.to_string(),
            });
        }
        check_string_match(
            restart_event.recovered_topology_digest.as_str(),
            self.pretrain_stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .topology_digest
                .as_str(),
            "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].recovered_topology_digest",
        )?;
        check_string_match(
            restart_event.dataset_identity.as_str(),
            self.pretrain_stage_receipt.dataset_identity.as_str(),
            "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].dataset_identity",
        )?;
        check_string_match(
            restart_event.sampling_policy_id.as_str(),
            self.pretrain_stage_receipt.sampling_policy_id.as_str(),
            "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].sampling_policy_id",
        )?;
        check_string_match(
            restart_event.sampling_policy_version.as_str(),
            self.pretrain_stage_receipt.sampling_policy_version.as_str(),
            "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].sampling_policy_version",
        )?;
        if restart_event.optimizer_state_step_restored
            != self
                .pretrain_stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .step
                .unwrap_or_default()
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].optimizer_state_step_restored",
                ),
                expected: self
                    .pretrain_stage_receipt
                    .checkpoint_lineage
                    .promoted_checkpoint
                    .step
                    .unwrap_or_default()
                    .to_string(),
                actual: restart_event.optimizer_state_step_restored.to_string(),
            });
        }
        let restore_receipt = restart_event.restore_receipt.as_ref().ok_or(
            PsionTrustedClusterRunError::MissingField {
                field: String::from(
                    "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].restore_receipt",
                ),
            },
        )?;
        if restore_receipt.selected_manifest.shards.len() < 2 {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].restore_receipt.selected_manifest.shards",
                ),
                expected: String::from("at least 2 shards"),
                actual: restore_receipt.selected_manifest.shards.len().to_string(),
            });
        }
        if restore_receipt.uploader_assignments.len()
            != usize::from(self.topology_contract.required_worker_count)
        {
            return Err(PsionTrustedClusterRunError::FieldMismatch {
                field: String::from(
                    "trusted_cluster_run_bundle.checkpoint_recovery_bundle.recovery_events[].restore_receipt.uploader_assignments",
                ),
                expected: self.topology_contract.required_worker_count.to_string(),
                actual: restore_receipt.uploader_assignments.len().to_string(),
            });
        }
        check_string_match(
            source_artifact
                .checkpoint_context
                .training_hardware_topology_digest
                .as_str(),
            self.observability_receipt
                .hardware_topology
                .topology_digest
                .as_str(),
            "trusted_cluster_run_bundle.checkpoint_recovery_bundle.checkpoint_artifacts[].checkpoint_context.training_hardware_topology_digest",
        )?;
        Ok(())
    }
}

/// Records one trusted-cluster distributed-group receipt and computes its stable digest.
#[allow(clippy::too_many_arguments)]
pub fn record_psion_trusted_cluster_distributed_group_receipt(
    group_id: impl Into<String>,
    kind: PsionTrustedClusterDistributedGroupKind,
    requested_backend: PsionTrustedClusterDistributedBackendFamily,
    resolved_backend: PsionTrustedClusterDistributedBackendFamily,
    effective_backend: impl Into<String>,
    communication_class: ClusterCommunicationClass,
    transport: ClusterTransportClass,
    topology_profile: PsionTrustedClusterDistributedTopologyProfile,
    mesh_id: impl Into<String>,
    mesh_revision: u64,
    local_node_id: impl Into<String>,
    local_rank: usize,
    world_size: usize,
    member_node_ids: Vec<String>,
    summary: impl Into<String>,
) -> Result<PsionTrustedClusterDistributedGroupReceipt, PsionTrustedClusterRunError> {
    let mut receipt = PsionTrustedClusterDistributedGroupReceipt {
        group_id: group_id.into(),
        kind,
        requested_backend,
        resolved_backend,
        effective_backend: effective_backend.into(),
        communication_class,
        transport,
        topology_profile,
        mesh_id: mesh_id.into(),
        mesh_revision,
        local_node_id: local_node_id.into(),
        local_rank,
        world_size,
        member_node_ids,
        summary: summary.into(),
        group_digest: String::new(),
    };
    receipt.group_digest = stable_trusted_cluster_distributed_group_digest(&receipt);
    receipt.validate()?;
    Ok(receipt)
}

/// Records one trusted-cluster topology contract and computes its stable digest.
#[allow(clippy::too_many_arguments)]
pub fn record_psion_trusted_cluster_topology_contract(
    contract_id: impl Into<String>,
    supported_topology_label: impl Into<String>,
    required_backend: impl Into<String>,
    required_device_id_prefix: impl Into<String>,
    required_worker_count: u16,
    one_rank_per_node_required: bool,
    required_distributed_backend: PsionTrustedClusterDistributedBackendFamily,
    required_distributed_topology_profile: PsionTrustedClusterDistributedTopologyProfile,
    required_transport: ClusterTransportClass,
    required_cluster_disposition: psionic_runtime::ClusterExecutionDisposition,
    required_execution_topology_kind: ExecutionTopologyKind,
    required_communication_class: ClusterCommunicationClass,
    required_optimizer_kind: TrainingDistributedOptimizerKind,
    required_collective_kind: TrainingCollectiveKind,
    required_collective_quantization: TrainingCollectiveQuantization,
    mode_evaluations: Vec<PsionTrustedClusterModeEvaluation>,
    summary: impl Into<String>,
    stage_receipt: &PsionPretrainStageRunReceipt,
    observability_receipt: &PsionPretrainRunObservabilityReceipt,
    rented_cluster_runbook: &PsionRentedClusterRunbook,
    distributed_group: &PsionTrustedClusterDistributedGroupReceipt,
    optimizer_contract: &DistributedOptimizerContract,
) -> Result<PsionTrustedClusterTopologyContract, PsionTrustedClusterRunError> {
    let mut contract = PsionTrustedClusterTopologyContract {
        schema_version: String::from(PSION_TRUSTED_CLUSTER_TOPOLOGY_CONTRACT_SCHEMA_VERSION),
        contract_id: contract_id.into(),
        pretrain_stage_receipt_digest: stage_receipt.receipt_digest.clone(),
        observability_receipt_digest: observability_receipt.observability_digest.clone(),
        rented_cluster_runbook_digest: rented_cluster_runbook.runbook_digest.clone(),
        supported_topology_label: supported_topology_label.into(),
        required_backend: required_backend.into(),
        required_device_id_prefix: required_device_id_prefix.into(),
        required_worker_count,
        one_rank_per_node_required,
        required_distributed_backend,
        required_distributed_topology_profile,
        required_transport,
        required_cluster_disposition,
        required_execution_topology_kind,
        required_communication_class,
        required_optimizer_kind,
        required_collective_kind,
        required_collective_quantization,
        mode_evaluations,
        summary: summary.into(),
        contract_digest: String::new(),
    };
    contract.contract_digest = stable_trusted_cluster_topology_contract_digest(&contract);
    contract.validate_against_inputs(
        stage_receipt,
        observability_receipt,
        rented_cluster_runbook,
        distributed_group,
        optimizer_contract,
    )?;
    Ok(contract)
}

/// Records one trusted-cluster replay receipt and computes its stable digest.
pub fn record_psion_trusted_cluster_replay_receipt(
    receipt_id: impl Into<String>,
    successful_replays: u16,
    participating_node_ids: Vec<String>,
    baseline_replay: TrainingReplayReceipt,
    observed_replay: TrainingReplayReceipt,
    summary: impl Into<String>,
    topology_contract: &PsionTrustedClusterTopologyContract,
    distributed_group: &PsionTrustedClusterDistributedGroupReceipt,
) -> Result<PsionTrustedClusterReplayReceipt, PsionTrustedClusterRunError> {
    let verification = baseline_replay.verify_against(&observed_replay);
    let mut receipt = PsionTrustedClusterReplayReceipt {
        schema_version: String::from(PSION_TRUSTED_CLUSTER_REPLAY_RECEIPT_SCHEMA_VERSION),
        receipt_id: receipt_id.into(),
        topology_contract_digest: topology_contract.contract_digest.clone(),
        participating_node_ids,
        baseline_replay,
        observed_replay,
        exact_replay_observed: verification.disposition == ReplayVerificationDisposition::Match,
        verification,
        successful_replays,
        summary: summary.into(),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = stable_trusted_cluster_replay_receipt_digest(&receipt);
    receipt.validate_against_inputs(topology_contract, distributed_group)?;
    Ok(receipt)
}

/// Records one trusted-cluster run bundle and computes its stable digest.
#[allow(clippy::too_many_arguments)]
pub fn record_psion_trusted_cluster_run_bundle(
    bundle_id: impl Into<String>,
    pretrain_stage_receipt: PsionPretrainStageRunReceipt,
    observability_receipt: PsionPretrainRunObservabilityReceipt,
    rented_cluster_runbook: PsionRentedClusterRunbook,
    topology_contract: PsionTrustedClusterTopologyContract,
    distributed_group: PsionTrustedClusterDistributedGroupReceipt,
    optimizer_contract: DistributedOptimizerContract,
    distributed_step_receipt: DistributedOptimizerStepReceipt,
    checkpoint_recovery_bundle: PsionCheckpointRecoveryBundle,
    replay_receipt: PsionTrustedClusterReplayReceipt,
    summary: impl Into<String>,
) -> Result<PsionTrustedClusterRunBundle, PsionTrustedClusterRunError> {
    let mut bundle = PsionTrustedClusterRunBundle {
        schema_version: String::from(PSION_TRUSTED_CLUSTER_RUN_BUNDLE_SCHEMA_VERSION),
        bundle_id: bundle_id.into(),
        pretrain_stage_receipt,
        observability_receipt,
        rented_cluster_runbook,
        topology_contract,
        distributed_group,
        optimizer_contract,
        distributed_step_receipt,
        checkpoint_recovery_bundle,
        replay_receipt,
        summary: summary.into(),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_trusted_cluster_run_bundle_digest(&bundle);
    bundle.validate()?;
    Ok(bundle)
}

/// Error returned by the trusted-cluster Psion run contract.
#[derive(Debug, Error)]
pub enum PsionTrustedClusterRunError {
    /// One required string field was empty.
    #[error("psion trusted-cluster contract is missing `{field}`")]
    MissingField {
        /// Missing field name.
        field: String,
    },
    /// One schema version drifted from the expected contract.
    #[error(
        "psion trusted-cluster schema version mismatch: expected `{expected}`, found `{actual}`"
    )]
    SchemaVersionMismatch {
        /// Expected schema version.
        expected: String,
        /// Observed schema version.
        actual: String,
    },
    /// One field drifted from the expected value.
    #[error(
        "psion trusted-cluster field `{field}` mismatch: expected `{expected}`, found `{actual}`"
    )]
    FieldMismatch {
        /// Field name.
        field: String,
        /// Expected value.
        expected: String,
        /// Observed value.
        actual: String,
    },
    /// Worker count was not multi-host.
    #[error("psion trusted-cluster worker count must be at least 2, found {actual}")]
    InvalidWorkerCount {
        /// Observed worker count.
        actual: u16,
    },
    /// One topology mode was repeated.
    #[error("psion trusted-cluster topology mode `{topology_mode}` was defined more than once")]
    DuplicateTopologyMode {
        /// Repeated topology mode.
        topology_mode: String,
    },
    /// The first trusted-cluster topology coverage was incomplete.
    #[error(
        "psion trusted-cluster topology coverage must include the supported homogeneous lane and at least one refused or out-of-scope mode"
    )]
    TopologyCoverageMissing {
        /// Whether the supported homogeneous lane was present.
        supported_lane_present: bool,
        /// Whether any refused or out-of-scope coverage existed.
        refused_or_out_of_scope_present: bool,
    },
    /// One selected device drifted outside the required device family prefix.
    #[error(
        "psion trusted-cluster device `{stable_device_id}` does not satisfy required prefix `{expected_prefix}`"
    )]
    DeviceIdPrefixMismatch {
        /// Observed stable device identifier.
        stable_device_id: String,
        /// Required prefix.
        expected_prefix: String,
    },
    /// The broader run omitted the explicit execution topology.
    #[error("psion trusted-cluster broader run omitted the explicit execution topology")]
    MissingExecutionTopology,
    /// The broader run omitted clustered execution facts.
    #[error("psion trusted-cluster broader run omitted clustered execution facts")]
    MissingClusterExecution,
    /// The broader run omitted the training collective context.
    #[error("psion trusted-cluster broader run omitted the training collective context")]
    MissingTrainingCollective,
    /// The broader run omitted the training recovery context.
    #[error("psion trusted-cluster broader run omitted the training recovery context")]
    MissingTrainingRecovery,
    /// The distributed group kind was outside the first truthful lane.
    #[error(
        "psion trusted-cluster distributed group must be `bootstrapped_mesh`, found `{actual:?}`"
    )]
    UnsupportedDistributedGroupKind {
        /// Actual group kind.
        actual: PsionTrustedClusterDistributedGroupKind,
    },
    /// The optimizer plan did not include one global sync stage.
    #[error("psion trusted-cluster optimizer plan omitted the global sync stage")]
    MissingGlobalSyncStage,
    /// The replay coverage did not reach the trusted-cluster minimum.
    #[error(
        "psion trusted-cluster replay coverage must be at least {expected_minimum}, found {actual}"
    )]
    ReplayCoverageTooSmall {
        /// Minimum trusted-cluster replay count.
        expected_minimum: u16,
        /// Actual replay count.
        actual: u16,
    },
    /// The replay verification drifted instead of matching exactly.
    #[error("psion trusted-cluster replay verification drifted")]
    ReplayVerificationDrifted,
    /// The broader run omitted distributed restart coverage.
    #[error("psion trusted-cluster run bundle omitted distributed restart coverage")]
    MissingDistributedRestartCoverage,
    /// The topology-contract digest did not recompute.
    #[error("psion trusted-cluster topology-contract digest did not recompute")]
    ContractDigestMismatch,
    /// One receipt or bundle digest did not recompute.
    #[error("psion trusted-cluster `{receipt_kind}` digest did not recompute")]
    ReceiptDigestMismatch {
        /// Receipt or bundle kind.
        receipt_kind: String,
    },
    /// Run observability validation failed.
    #[error(transparent)]
    ObservabilityContract(#[from] PsionPretrainRunObservabilityError),
    /// Checkpoint recovery validation failed.
    #[error(transparent)]
    CheckpointRecoveryContract(#[from] PsionCheckpointRecoveryContractError),
}

fn validate_optimizer_plan_alignment(
    optimizer_contract: &DistributedOptimizerContract,
    distributed_group: &PsionTrustedClusterDistributedGroupReceipt,
    training_collective: &psionic_runtime::TrainingCollectiveContext,
    required_communication_class: ClusterCommunicationClass,
    required_collective_kind: TrainingCollectiveKind,
    required_collective_quantization: TrainingCollectiveQuantization,
) -> Result<(), PsionTrustedClusterRunError> {
    if optimizer_contract.collective_sync_plan.stages.is_empty() {
        return Err(PsionTrustedClusterRunError::MissingGlobalSyncStage);
    }
    let global_stage = optimizer_contract
        .collective_sync_plan
        .stages
        .iter()
        .find(|stage| stage.scope == CollectiveSyncScope::GlobalMesh)
        .ok_or(PsionTrustedClusterRunError::MissingGlobalSyncStage)?;
    compare_string_sets(
        global_stage.member_node_ids.as_slice(),
        distributed_group.member_node_ids.as_slice(),
        "trusted_cluster_topology_contract.optimizer_contract.collective_sync_plan.stages[].member_node_ids",
    )?;
    let collective = &global_stage.plan.collective;
    if collective.kind != required_collective_kind {
        return Err(PsionTrustedClusterRunError::FieldMismatch {
            field: String::from(
                "trusted_cluster_topology_contract.optimizer_contract.collective_sync_plan.stages[].plan.collective.kind",
            ),
            expected: format!("{:?}", required_collective_kind),
            actual: format!("{:?}", collective.kind),
        });
    }
    if collective.quantization != required_collective_quantization {
        return Err(PsionTrustedClusterRunError::FieldMismatch {
            field: String::from(
                "trusted_cluster_topology_contract.optimizer_contract.collective_sync_plan.stages[].plan.collective.quantization",
            ),
            expected: format!("{:?}", required_collective_quantization),
            actual: format!("{:?}", collective.quantization),
        });
    }
    if collective.worker_count != distributed_group.world_size {
        return Err(PsionTrustedClusterRunError::FieldMismatch {
            field: String::from(
                "trusted_cluster_topology_contract.optimizer_contract.collective_sync_plan.stages[].plan.collective.worker_count",
            ),
            expected: distributed_group.world_size.to_string(),
            actual: collective.worker_count.to_string(),
        });
    }
    check_string_match(
        collective.device_mesh.mesh_id.as_str(),
        distributed_group.mesh_id.as_str(),
        "trusted_cluster_topology_contract.optimizer_contract.collective_sync_plan.stages[].plan.collective.device_mesh.mesh_id",
    )?;
    if collective.device_mesh.mesh_revision != distributed_group.mesh_revision {
        return Err(PsionTrustedClusterRunError::FieldMismatch {
            field: String::from(
                "trusted_cluster_topology_contract.optimizer_contract.collective_sync_plan.stages[].plan.collective.device_mesh.mesh_revision",
            ),
            expected: distributed_group.mesh_revision.to_string(),
            actual: collective.device_mesh.mesh_revision.to_string(),
        });
    }
    if collective.device_mesh.communication_class != required_communication_class {
        return Err(PsionTrustedClusterRunError::FieldMismatch {
            field: String::from(
                "trusted_cluster_topology_contract.optimizer_contract.collective_sync_plan.stages[].plan.collective.device_mesh.communication_class",
            ),
            expected: format!("{:?}", required_communication_class),
            actual: format!("{:?}", collective.device_mesh.communication_class),
        });
    }
    compare_string_sets(
        collective.device_mesh.member_node_ids.as_slice(),
        training_collective.device_mesh.member_node_ids.as_slice(),
        "trusted_cluster_topology_contract.optimizer_contract.collective_sync_plan.stages[].plan.collective.device_mesh.member_node_ids",
    )?;
    Ok(())
}

fn baseline_replay_worker_ids(
    replay_receipt: &TrainingReplayReceipt,
) -> Result<Vec<String>, PsionTrustedClusterRunError> {
    let mut workers = replay_receipt
        .sample_selection_rules
        .iter()
        .map(|rule| rule.worker_id.clone())
        .collect::<Vec<_>>();
    workers.sort();
    workers.dedup();
    if workers.is_empty() {
        return Err(PsionTrustedClusterRunError::MissingField {
            field: String::from("trusted_cluster_replay_receipt.sample_selection_rules"),
        });
    }
    Ok(workers)
}

fn ensure_nonempty(value: &str, field: &str) -> Result<(), PsionTrustedClusterRunError> {
    if value.trim().is_empty() {
        return Err(PsionTrustedClusterRunError::MissingField {
            field: field.to_owned(),
        });
    }
    Ok(())
}

fn ensure_device_prefix(
    stable_device_id: &str,
    expected_prefix: &str,
) -> Result<(), PsionTrustedClusterRunError> {
    if !stable_device_id.starts_with(expected_prefix) {
        return Err(PsionTrustedClusterRunError::DeviceIdPrefixMismatch {
            stable_device_id: stable_device_id.to_owned(),
            expected_prefix: expected_prefix.to_owned(),
        });
    }
    Ok(())
}

fn check_string_match(
    actual: &str,
    expected: &str,
    field: &str,
) -> Result<(), PsionTrustedClusterRunError> {
    if actual != expected {
        return Err(PsionTrustedClusterRunError::FieldMismatch {
            field: field.to_owned(),
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        });
    }
    Ok(())
}

fn compare_string_sets(
    actual: &[String],
    expected: &[String],
    field: &str,
) -> Result<(), PsionTrustedClusterRunError> {
    let mut actual_sorted = actual.to_vec();
    let mut expected_sorted = expected.to_vec();
    actual_sorted.sort();
    actual_sorted.dedup();
    expected_sorted.sort();
    expected_sorted.dedup();
    if actual_sorted != expected_sorted {
        return Err(PsionTrustedClusterRunError::FieldMismatch {
            field: field.to_owned(),
            expected: serde_json::to_string(&expected_sorted).unwrap_or_default(),
            actual: serde_json::to_string(&actual_sorted).unwrap_or_default(),
        });
    }
    Ok(())
}

fn distinct_count(values: &[String]) -> usize {
    values.iter().collect::<BTreeSet<_>>().len()
}

fn trusted_cluster_topology_mode_name(mode: PsionTrustedClusterTopologyMode) -> &'static str {
    match mode {
        PsionTrustedClusterTopologyMode::HomogeneousCudaH100OneRankPerNode => {
            "homogeneous_cuda_h100_one_rank_per_node"
        }
        PsionTrustedClusterTopologyMode::MixedBackendOrAccelerator => {
            "mixed_backend_or_accelerator"
        }
        PsionTrustedClusterTopologyMode::CrossRegionOrMixedLatencyFabric => {
            "cross_region_or_mixed_latency_fabric"
        }
        PsionTrustedClusterTopologyMode::ElasticWorldSizeChange => "elastic_world_size_change",
        PsionTrustedClusterTopologyMode::SharedOrUntrustedFabric => "shared_or_untrusted_fabric",
    }
}

fn stable_trusted_cluster_topology_contract_digest(
    contract: &PsionTrustedClusterTopologyContract,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_trusted_cluster_topology_contract|");
    hasher.update(contract.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(contract.contract_id.as_bytes());
    hasher.update(b"|");
    hasher.update(contract.pretrain_stage_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(contract.observability_receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(contract.rented_cluster_runbook_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(contract.supported_topology_label.as_bytes());
    hasher.update(b"|");
    hasher.update(contract.required_backend.as_bytes());
    hasher.update(b"|");
    hasher.update(contract.required_device_id_prefix.as_bytes());
    hasher.update(b"|");
    hasher.update(contract.required_worker_count.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(if contract.one_rank_per_node_required {
        b"1"
    } else {
        b"0"
    });
    hasher.update(b"|");
    hasher.update(stable_json_bytes(&contract.required_distributed_backend));
    hasher.update(stable_json_bytes(
        &contract.required_distributed_topology_profile,
    ));
    hasher.update(stable_json_bytes(&contract.required_transport));
    hasher.update(stable_json_bytes(&contract.required_cluster_disposition));
    hasher.update(stable_json_bytes(
        &contract.required_execution_topology_kind,
    ));
    hasher.update(stable_json_bytes(&contract.required_communication_class));
    hasher.update(stable_json_bytes(&contract.required_optimizer_kind));
    hasher.update(stable_json_bytes(&contract.required_collective_kind));
    hasher.update(stable_json_bytes(
        &contract.required_collective_quantization,
    ));
    for evaluation in &contract.mode_evaluations {
        hasher.update(b"|mode|");
        hasher.update(stable_json_bytes(evaluation));
    }
    hasher.update(b"|");
    hasher.update(contract.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_trusted_cluster_distributed_group_digest(
    receipt: &PsionTrustedClusterDistributedGroupReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_trusted_cluster_distributed_group|");
    hasher.update(receipt.group_id.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_json_bytes(&receipt.kind));
    hasher.update(stable_json_bytes(&receipt.requested_backend));
    hasher.update(stable_json_bytes(&receipt.resolved_backend));
    hasher.update(receipt.effective_backend.as_bytes());
    hasher.update(stable_json_bytes(&receipt.communication_class));
    hasher.update(stable_json_bytes(&receipt.transport));
    hasher.update(stable_json_bytes(&receipt.topology_profile));
    hasher.update(receipt.mesh_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.mesh_revision.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.local_node_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.local_rank.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.world_size.to_string().as_bytes());
    for node_id in &receipt.member_node_ids {
        hasher.update(b"|member|");
        hasher.update(node_id.as_bytes());
    }
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_trusted_cluster_replay_receipt_digest(
    receipt: &PsionTrustedClusterReplayReceipt,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_trusted_cluster_replay_receipt|");
    hasher.update(receipt.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.topology_contract_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.successful_replays.to_string().as_bytes());
    hasher.update(b"|");
    hasher.update(if receipt.exact_replay_observed {
        b"1"
    } else {
        b"0"
    });
    hasher.update(b"|");
    for node_id in &receipt.participating_node_ids {
        hasher.update(node_id.as_bytes());
        hasher.update(b"|");
    }
    hasher.update(receipt.baseline_replay.replay_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(receipt.observed_replay.replay_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_json_bytes(&receipt.verification));
    hasher.update(b"|");
    hasher.update(receipt.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_trusted_cluster_run_bundle_digest(bundle: &PsionTrustedClusterRunBundle) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"psion_trusted_cluster_run_bundle|");
    hasher.update(bundle.schema_version.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.bundle_id.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.pretrain_stage_receipt.receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.observability_receipt.observability_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.rented_cluster_runbook.runbook_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.topology_contract.contract_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(stable_json_bytes(&bundle.distributed_group));
    hasher.update(b"|");
    hasher.update(bundle.optimizer_contract.contract_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.distributed_step_receipt.receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.checkpoint_recovery_bundle.bundle_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.replay_receipt.receipt_digest.as_bytes());
    hasher.update(b"|");
    hasher.update(bundle.summary.as_bytes());
    hex::encode(hasher.finalize())
}

fn stable_json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use psionic_runtime::ClusterTransportClass;

    use super::*;

    #[test]
    fn trusted_cluster_run_bundle_fixture_validates() -> Result<(), Box<dyn std::error::Error>> {
        let bundle = fixture_bundle()?;
        bundle.validate()?;
        Ok(())
    }

    #[test]
    fn trusted_cluster_run_bundle_rejects_transport_drift() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut bundle = fixture_bundle()?;
        bundle.distributed_group.transport = ClusterTransportClass::TrustedLanDatagram;
        let error = bundle
            .validate()
            .expect_err("transport drift should be rejected");
        assert!(
            matches!(
                &error,
                PsionTrustedClusterRunError::FieldMismatch { field, .. }
                if field == "trusted_cluster_topology_contract.distributed_group.transport"
            ) || matches!(
                &error,
                PsionTrustedClusterRunError::ReceiptDigestMismatch { receipt_kind }
                if receipt_kind == "trusted_cluster_distributed_group"
            )
        );
        Ok(())
    }

    #[test]
    fn trusted_cluster_replay_receipt_requires_full_worker_coverage(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut bundle = fixture_bundle()?;
        bundle.replay_receipt.participating_node_ids.pop();
        let error = bundle
            .validate()
            .expect_err("worker coverage drift should be rejected");
        assert!(matches!(
            error,
            PsionTrustedClusterRunError::FieldMismatch { field, .. }
            if field == "trusted_cluster_replay_receipt.participating_node_ids"
        ));
        Ok(())
    }

    #[test]
    fn trusted_cluster_run_bundle_requires_distributed_restart_coverage(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut bundle = fixture_bundle()?;
        bundle
            .checkpoint_recovery_bundle
            .recovery_events
            .retain(|event| {
                event.event_kind != PsionCheckpointRecoveryEventKind::DistributedRestart
            });
        let error = bundle
            .validate()
            .expect_err("missing distributed restart should be rejected");
        assert!(matches!(
            error,
            PsionTrustedClusterRunError::CheckpointRecoveryContract(
                PsionCheckpointRecoveryContractError::MissingRecoveryEventKind { .. }
            ) | PsionTrustedClusterRunError::MissingDistributedRestartCoverage
        ));
        Ok(())
    }

    fn fixture_bundle() -> Result<PsionTrustedClusterRunBundle, Box<dyn std::error::Error>> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../")
            .join("fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json");
        Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
    }
}
