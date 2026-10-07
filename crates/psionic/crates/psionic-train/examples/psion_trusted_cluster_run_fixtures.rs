use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

use psionic_cluster::NodeId;
use psionic_collectives::{
    CollectiveMeshMember, CollectiveSyncCadencePolicy, CollectiveSyncCadenceReceipt,
    CollectiveTransportFeedback, ElasticCollectivePlanner, QuantizedCollectiveBenchmark,
    QuantizedCollectiveBenchmarkSample,
};
use psionic_core::{DType, Device, Shape, TensorSpec};
use psionic_data::{DatasetSplitKind, PsionTokenizedCorpusManifest};
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifestRef, DatastreamSubjectKind,
};
use psionic_environments::{
    EnvironmentExecutionEntrypoint, EnvironmentPackageContract, EnvironmentPackageFamily,
    EnvironmentPackageKey, EnvironmentRuntimeFamily, EnvironmentStateMode, EnvironmentToolContract,
    EnvironmentToolInterface,
};
use psionic_eval::{EvalExecutionStrategyFacts, EvalRunContract, EvalRunMode};
use psionic_models::PsionCompactDecoderDescriptor;
use psionic_runtime::{
    ClusterCommunicationClass, ClusterExecutionContext, ClusterExecutionDisposition,
    ClusterSelectedNode, ClusterTransportClass, DeliveredExecutionContext,
    DeviceInventoryQualifiers, DeviceMemoryClass, DevicePerformanceClass, ExecutionTopologyPlan,
    TrainingCheckpointAvailability, TrainingCheckpointReference, TrainingCollectiveContext,
    TrainingCollectiveKind, TrainingCollectiveQuantization, TrainingDeviceMeshAxis,
    TrainingDeviceMeshAxisKind, TrainingDeviceMeshContext, TrainingElasticMembershipContext,
    TrainingRecoveryContext, TrainingRecoveryPosture,
};
use psionic_train::{
    record_psion_checkpoint_artifact, record_psion_checkpoint_corruption,
    record_psion_checkpoint_recovery_bundle, record_psion_checkpoint_recovery_event,
    record_psion_pretrain_run_observability,
    record_psion_trusted_cluster_distributed_group_receipt,
    record_psion_trusted_cluster_replay_receipt, record_psion_trusted_cluster_run_bundle,
    record_psion_trusted_cluster_topology_contract, run_psion_pretrain_stage,
    CheckpointDurabilityPosture, CheckpointManifest, CheckpointPointer, CheckpointScopeBinding,
    CheckpointScopeKind, CheckpointShardManifest, CheckpointStoreReadOptions,
    DeterministicSampleSelectionRule, DistributedOptimizerContract,
    DistributedOptimizerGroupContract, DistributedOptimizerRun, DistributedTrainingMemoryBudget,
    InMemoryCheckpointStore, OptimizerStateResidency, PolicyRevision,
    PsionCheckpointContextReceipt, PsionCheckpointCorruptionKind, PsionCheckpointLayoutKind,
    PsionCheckpointRecoveryDisposition, PsionCheckpointRecoveryEventKind,
    PsionPretrainCheckpointArtifactReceipt, PsionPretrainCheckpointLineageReceipt,
    PsionPretrainHardwareTopologyReceipt, PsionPretrainLossNormalization,
    PsionPretrainObjectiveConfig, PsionPretrainObjectiveKind, PsionPretrainReplayReceipt,
    PsionPretrainRunCostBasis, PsionPretrainRunCostReceipt, PsionPretrainRunScaleProfile,
    PsionPretrainRunThroughputReceipt, PsionPretrainSourceFamilyReportRow,
    PsionPretrainStageConfig, PsionPretrainStageRunReceipt, PsionRentedClusterRunbook,
    PsionSamplingPolicyManifest, PsionTrustedClusterDistributedBackendFamily,
    PsionTrustedClusterDistributedGroupKind, PsionTrustedClusterDistributedTopologyProfile,
    PsionTrustedClusterModeEvaluation, PsionTrustedClusterTopologyDisposition,
    PsionTrustedClusterTopologyMode, ReplayEnvironmentPin, ReproducibleEvalPosture,
    RolloutArtifact, RolloutProofKind, RolloutProofReference, RolloutSample,
    RolloutTerminationReason, TrainerBatch, TrainingActivationCheckpointPolicy,
    TrainingGradientAccumulationPolicy, TrainingGradientAccumulationReduction,
    TrainingGradientBatch, TrainingInstabilityPolicy, TrainingInstabilityRule,
    TrainingInstabilitySignalKind, TrainingInstabilityTelemetry, TrainingLoopBudget,
    TrainingOperationalAction, TrainingOptimizerConfig, TrainingOptimizerResidencyPolicy,
    TrainingOptimizerShardResidency, TrainingOptimizerStateShardKind,
    TrainingOptimizerStateShardLayout, TrainingParameterClass, TrainingParameterGroupState,
    TrainingParameterShardKind, TrainingParameterShardLayout, TrainingPrecisionPolicy,
    TrainingRecoveryMode, TrainingReplayReceipt, TrainingReplaySeedDiscipline,
    TrainingRiskyOptimization, TrainingRiskyOptimizationRule, TrainingShardPlacement,
    TrainingShardRange, TrainingStabilityController, TrainingStabilityVerdict,
    TrainingTensorBuffer,
};
use serde_json::json;

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/trusted_cluster");
    fs::create_dir_all(&fixtures_dir)?;

    let rented_cluster_runbook: PsionRentedClusterRunbook =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/rented_cluster/psion_rented_cluster_runbook_v1.json"),
        )?)?;

    let stage_receipt = broader_stage_receipt(&root)?;
    let observability_receipt = broader_run_observability(&stage_receipt)?;
    let distributed_group = distributed_group_receipt(&observability_receipt)?;
    let optimizer_contract = distributed_optimizer_contract()?;
    let distributed_step_receipt = distributed_optimizer_step_receipt(&optimizer_contract)?;
    let checkpoint_recovery_bundle =
        trusted_cluster_checkpoint_recovery_bundle(&stage_receipt, &observability_receipt)?;
    let topology_contract = record_psion_trusted_cluster_topology_contract(
        "psion-trusted-cluster-topology-contract-v1",
        "homogeneous_four_node_h100_tensor_parallel",
        "cuda",
        "cuda:h100-",
        4,
        true,
        PsionTrustedClusterDistributedBackendFamily::Nccl,
        PsionTrustedClusterDistributedTopologyProfile::TrustedLanStreamMesh,
        ClusterTransportClass::TrustedLanStream,
        ClusterExecutionDisposition::Sharded,
        psionic_runtime::ExecutionTopologyKind::TensorSharded,
        ClusterCommunicationClass::TensorCollectiveMesh,
        psionic_train::TrainingDistributedOptimizerKind::ZeroStage3,
        TrainingCollectiveKind::AllReduce,
        TrainingCollectiveQuantization::Int8Symmetric,
        trusted_cluster_mode_evaluations(),
        "Trusted-cluster Psion scale-up is bounded to one homogeneous four-node CUDA H100 tensor-parallel lane with one rank per node, trusted-LAN stream transport, explicit NCCL-class public group truth, and explicit refusal or out-of-scope posture for mixed, cross-region, shared, or elastic-world-size modes.",
        &stage_receipt,
        &observability_receipt,
        &rented_cluster_runbook,
        &distributed_group,
        &optimizer_contract,
    )?;
    let (baseline_replay, observed_replay) = trusted_cluster_replay_pair()?;
    let replay_receipt = record_psion_trusted_cluster_replay_receipt(
        "psion-trusted-cluster-replay-v1",
        5,
        node_ids(),
        baseline_replay,
        observed_replay,
        "Trusted-cluster replay receipts cover all four worker identities and preserve exact replay across repeated multi-host checks.",
        &topology_contract,
        &distributed_group,
    )?;
    let run_bundle = record_psion_trusted_cluster_run_bundle(
        "psion-trusted-cluster-run-bundle-v1",
        stage_receipt,
        observability_receipt,
        rented_cluster_runbook,
        topology_contract.clone(),
        distributed_group,
        optimizer_contract,
        distributed_step_receipt,
        checkpoint_recovery_bundle.clone(),
        replay_receipt.clone(),
        "The first bounded trusted-cluster Psion run bundle ties together the broader-stage receipt, broader-run throughput and topology receipts, bootstrapped public distributed group truth, optimizer-step evidence, multi-host replay verification, and checkpoint restart coverage on one explicit four-node trusted cluster lane.",
    )?;

    fs::write(
        fixtures_dir.join("psion_trusted_cluster_topology_contract_v1.json"),
        serde_json::to_vec_pretty(&topology_contract)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_trusted_cluster_replay_receipt_v1.json"),
        serde_json::to_vec_pretty(&replay_receipt)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_trusted_cluster_checkpoint_recovery_bundle_v1.json"),
        serde_json::to_vec_pretty(&checkpoint_recovery_bundle)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_trusted_cluster_run_bundle_v1.json"),
        serde_json::to_vec_pretty(&run_bundle)?,
    )?;
    Ok(())
}

fn trusted_cluster_mode_evaluations() -> Vec<PsionTrustedClusterModeEvaluation> {
    vec![
        PsionTrustedClusterModeEvaluation {
            topology_mode: PsionTrustedClusterTopologyMode::HomogeneousCudaH100OneRankPerNode,
            disposition: PsionTrustedClusterTopologyDisposition::Supported,
            detail: String::from(
                "The first trusted-cluster lane is one homogeneous H100-class CUDA rank per node over trusted-LAN stream transport.",
            ),
        },
        PsionTrustedClusterModeEvaluation {
            topology_mode: PsionTrustedClusterTopologyMode::MixedBackendOrAccelerator,
            disposition: PsionTrustedClusterTopologyDisposition::Refused,
            detail: String::from(
                "Mixed backend or accelerator meshes are refused instead of pretending collective and throughput behavior generalize automatically.",
            ),
        },
        PsionTrustedClusterModeEvaluation {
            topology_mode: PsionTrustedClusterTopologyMode::CrossRegionOrMixedLatencyFabric,
            disposition: PsionTrustedClusterTopologyDisposition::Refused,
            detail: String::from(
                "Cross-region or mixed-latency fabrics are refused because the first truthful lane is bounded to one trusted low-latency cluster.",
            ),
        },
        PsionTrustedClusterModeEvaluation {
            topology_mode: PsionTrustedClusterTopologyMode::ElasticWorldSizeChange,
            disposition: PsionTrustedClusterTopologyDisposition::OutOfScope,
            detail: String::from(
                "Elastic world-size increase remains out of scope for the first trusted-cluster lane; recovery is same-world-size restart only.",
            ),
        },
        PsionTrustedClusterModeEvaluation {
            topology_mode: PsionTrustedClusterTopologyMode::SharedOrUntrustedFabric,
            disposition: PsionTrustedClusterTopologyDisposition::Refused,
            detail: String::from(
                "Shared or untrusted fabrics are refused rather than widening this lane into public all-reduce claims.",
            ),
        },
    ]
}

fn distributed_group_receipt(
    observability_receipt: &psionic_train::PsionPretrainRunObservabilityReceipt,
) -> Result<psionic_train::PsionTrustedClusterDistributedGroupReceipt, Box<dyn Error>> {
    let cluster_execution = observability_receipt
        .hardware_topology
        .delivered_execution
        .cluster_execution
        .as_ref()
        .ok_or_else(|| String::from("broader run should expose cluster execution"))?;
    let training_collective = cluster_execution
        .training_collective
        .as_ref()
        .ok_or_else(|| String::from("broader run should expose training collective"))?;
    Ok(record_psion_trusted_cluster_distributed_group_receipt(
        "psion-trusted-cluster-group-v1",
        PsionTrustedClusterDistributedGroupKind::BootstrappedMesh,
        PsionTrustedClusterDistributedBackendFamily::Nccl,
        PsionTrustedClusterDistributedBackendFamily::Nccl,
        "cuda",
        ClusterCommunicationClass::TensorCollectiveMesh,
        ClusterTransportClass::TrustedLanStream,
        PsionTrustedClusterDistributedTopologyProfile::TrustedLanStreamMesh,
        training_collective.device_mesh.mesh_id.clone(),
        training_collective.device_mesh.mesh_revision,
        "worker-c",
        2,
        4,
        node_ids(),
        "Train-owned distributed-group receipt freezes the bounded NCCL-class trusted-LAN stream mesh that the trusted-cluster lane requires from public distributed-group truth.",
    )?)
}

fn distributed_optimizer_contract() -> Result<DistributedOptimizerContract, Box<dyn Error>> {
    let collective_sync_plan = trusted_collective_sync_plan()?;
    Ok(DistributedOptimizerContract::new(
        "optimizer://psion-trusted-cluster",
        psionic_train::TrainingDistributedOptimizerKind::ZeroStage3,
        TrainingPrecisionPolicy::bf16_master_fp32(TrainingCollectiveQuantization::Int8Symmetric),
        TrainingGradientAccumulationPolicy::new(
            2,
            TrainingGradientAccumulationReduction::Mean,
            TrainingCollectiveKind::AllReduce,
        ),
        TrainingActivationCheckpointPolicy::EveryNthBlock {
            block_interval: 2,
            activation_peak_bytes_without_checkpointing: 14_400_000_000,
            activation_peak_bytes_with_checkpointing: 6_200_000_000,
            rematerialization_overhead_bps: 900,
        },
        DistributedTrainingMemoryBudget::new(
            72 * 1024 * 1024 * 1024,
            16 * 1024 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
        ),
        collective_sync_plan,
        vec![
            DistributedOptimizerGroupContract::new(
                "decoder.weight",
                TrainingParameterClass::Matrix,
                full_shard_layout("tp"),
                full_shard_layout("tp"),
                TrainingOptimizerStateShardLayout::new(
                    TrainingOptimizerStateShardKind::ZeroStage3,
                    TrainingOptimizerShardResidency::HostOffloaded,
                    full_shard_placements("tp"),
                )
                .with_axis_id("tp"),
                OptimizerStateResidency::HostResident,
            ),
            DistributedOptimizerGroupContract::new(
                "decoder.bias",
                TrainingParameterClass::Bias,
                replicated_layout("tp"),
                replicated_layout("tp"),
                TrainingOptimizerStateShardLayout::new(
                    TrainingOptimizerStateShardKind::Replicated,
                    TrainingOptimizerShardResidency::DeviceResident,
                    replicated_placements("tp"),
                )
                .with_axis_id("tp"),
                OptimizerStateResidency::DeviceResident,
            ),
        ],
    )?)
}

fn distributed_optimizer_step_receipt(
    optimizer_contract: &DistributedOptimizerContract,
) -> Result<psionic_train::DistributedOptimizerStepReceipt, Box<dyn Error>> {
    let mut run = DistributedOptimizerRun::new(
        "psion-trusted-cluster-distributed-run",
        "train.psion.decoder",
        TrainingLoopBudget::new(2, 1, 1)?,
        parameter_groups()?,
        optimizer_contract.clone(),
    )?;
    run.record_microbatch(microbatch(
        "microbatch-a",
        [0.01, 0.02, 0.03, 0.04],
        [0.004, 0.005, 0.006, 0.007],
    )?)?;
    run.record_microbatch(microbatch(
        "microbatch-b",
        [0.015, 0.025, 0.035, 0.045],
        [0.005, 0.006, 0.007, 0.008],
    )?)?;
    Ok(run.apply_accumulated_step(1_742_620_100_000, 1_742_620_100_240)?)
}

fn trusted_collective_sync_plan(
) -> Result<psionic_collectives::CollectiveSyncExecutionPlan, Box<dyn Error>> {
    let mut planner = ElasticCollectivePlanner::new(
        "psion-broad-mesh",
        "cuda",
        ClusterCommunicationClass::TensorCollectiveMesh,
        vec![
            TrainingDeviceMeshAxis::new("tp", TrainingDeviceMeshAxisKind::TensorParallel, 4)
                .with_collective_group_size(4)
                .with_detail("one tensor-parallel shard per trusted-cluster worker"),
        ],
    );
    let membership = broader_membership();
    planner.observe_mesh(
        membership.clone(),
        vec![
            CollectiveMeshMember::new("worker-a", 0, 0, "cuda:h100-0"),
            CollectiveMeshMember::new("worker-b", 1, 1, "cuda:h100-1"),
            CollectiveMeshMember::new("worker-c", 2, 2, "cuda:h100-2"),
            CollectiveMeshMember::new("worker-d", 3, 3, "cuda:h100-3"),
        ],
    )?;
    planner.record_benchmark(QuantizedCollectiveBenchmark::new(
        TrainingCollectiveKind::AllReduce,
        TrainingCollectiveQuantization::Int8Symmetric,
        QuantizedCollectiveBenchmarkSample::new(2_400, 32 * 1024 * 1024, 0),
        QuantizedCollectiveBenchmarkSample::new(1_200, 8 * 1024 * 1024, 12),
        100,
        1_000,
    ));
    planner.observe_transport_feedback(
        CollectiveTransportFeedback::new(1_742_620_090_000, 6_400, 1, 1)
            .with_detail("trusted-cluster stream fabric stayed healthy during the bound sync"),
    );
    let mut plan = planner.plan_sync(
        16_384,
        TrainingCollectiveKind::AllReduce,
        512 * 1024 * 1024,
        TrainingCollectiveQuantization::Int8Symmetric,
        &CollectiveSyncCadencePolicy::new(),
    )?;
    let canonical_mesh = TrainingDeviceMeshContext::new(
        "psion-broad-mesh",
        7,
        "cuda",
        ClusterCommunicationClass::TensorCollectiveMesh,
        membership,
        node_ids(),
    )
    .with_axes(vec![TrainingDeviceMeshAxis::new(
        "tp",
        TrainingDeviceMeshAxisKind::TensorParallel,
        4,
    )
    .with_collective_group_size(4)
    .with_detail("one tensor-parallel shard per worker")]);
    plan.cadence_receipt = CollectiveSyncCadenceReceipt::new(
        plan.cadence_receipt.step_index,
        7,
        plan.cadence_receipt.cadence_class,
        plan.cadence_receipt.global_interval_steps,
        plan.cadence_receipt.next_global_step,
        plan.cadence_receipt.local_group_size,
        plan.cadence_receipt.global_quantization,
        plan.cadence_receipt.local_quantization,
        plan.cadence_receipt.degraded_transport,
        plan.cadence_receipt.triggers.clone(),
        plan.cadence_receipt.transport_feedback.clone(),
    );
    for stage in &mut plan.stages {
        stage.member_node_ids = node_ids();
        stage.plan.collective.device_mesh = canonical_mesh.clone();
    }
    Ok(plan)
}

fn full_shard_layout(axis_id: &str) -> TrainingParameterShardLayout {
    TrainingParameterShardLayout::new(
        TrainingParameterShardKind::FullShard,
        full_shard_placements(axis_id),
    )
    .with_axis_id(axis_id)
}

fn replicated_layout(axis_id: &str) -> TrainingParameterShardLayout {
    TrainingParameterShardLayout::new(
        TrainingParameterShardKind::Replicated,
        replicated_placements(axis_id),
    )
    .with_axis_id(axis_id)
}

fn full_shard_placements(axis_id: &str) -> Vec<TrainingShardPlacement> {
    vec![
        TrainingShardPlacement::new(
            0,
            axis_id,
            "worker-a",
            "cuda:h100-0",
            0,
            TrainingShardRange::new(0, 1),
        ),
        TrainingShardPlacement::new(
            1,
            axis_id,
            "worker-b",
            "cuda:h100-1",
            0,
            TrainingShardRange::new(1, 1),
        ),
        TrainingShardPlacement::new(
            2,
            axis_id,
            "worker-c",
            "cuda:h100-2",
            0,
            TrainingShardRange::new(2, 1),
        ),
        TrainingShardPlacement::new(
            3,
            axis_id,
            "worker-d",
            "cuda:h100-3",
            0,
            TrainingShardRange::new(3, 1),
        ),
    ]
}

fn replicated_placements(axis_id: &str) -> Vec<TrainingShardPlacement> {
    vec![
        TrainingShardPlacement::new(
            0,
            axis_id,
            "worker-a",
            "cuda:h100-0",
            0,
            TrainingShardRange::new(0, 4),
        ),
        TrainingShardPlacement::new(
            1,
            axis_id,
            "worker-b",
            "cuda:h100-1",
            1,
            TrainingShardRange::new(0, 4),
        ),
        TrainingShardPlacement::new(
            2,
            axis_id,
            "worker-c",
            "cuda:h100-2",
            2,
            TrainingShardRange::new(0, 4),
        ),
        TrainingShardPlacement::new(
            3,
            axis_id,
            "worker-d",
            "cuda:h100-3",
            3,
            TrainingShardRange::new(0, 4),
        ),
    ]
}

fn parameter_groups() -> Result<Vec<TrainingParameterGroupState>, Box<dyn Error>> {
    Ok(vec![
        TrainingParameterGroupState::new(
            "decoder.weight",
            TrainingParameterClass::Matrix,
            TrainingTensorBuffer::from_f32(
                "decoder.weight",
                TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu()),
                vec![0.1, 0.2, 0.3, 0.4],
            )?,
            TrainingOptimizerConfig::adamw(0.0003, 0.9, 0.999, 1e-8),
            TrainingOptimizerResidencyPolicy::device_step_offload_idle(),
        )?,
        TrainingParameterGroupState::new(
            "decoder.bias",
            TrainingParameterClass::Bias,
            TrainingTensorBuffer::from_f32(
                "decoder.bias",
                TensorSpec::new(Shape::new(vec![4]), DType::F32, Device::cpu()),
                vec![0.0, 0.1, 0.2, 0.3],
            )?,
            TrainingOptimizerConfig::sgd(0.01).with_momentum(0.9),
            TrainingOptimizerResidencyPolicy::host_only(),
        )?,
    ])
}

fn microbatch(
    batch_id: &str,
    weight_gradient: [f32; 4],
    bias_gradient: [f32; 4],
) -> Result<TrainingGradientBatch, Box<dyn Error>> {
    Ok(TrainingGradientBatch::new(
        batch_id,
        0.42,
        8,
        BTreeMap::from([
            (
                String::from("decoder.weight"),
                TrainingTensorBuffer::from_f32(
                    "decoder.weight",
                    TensorSpec::new(Shape::new(vec![2, 2]), DType::F32, Device::cpu()),
                    weight_gradient.to_vec(),
                )?,
            ),
            (
                String::from("decoder.bias"),
                TrainingTensorBuffer::from_f32(
                    "decoder.bias",
                    TensorSpec::new(Shape::new(vec![4]), DType::F32, Device::cpu()),
                    bias_gradient.to_vec(),
                )?,
            ),
        ]),
    ))
}

fn trusted_cluster_checkpoint_recovery_bundle(
    stage_receipt: &PsionPretrainStageRunReceipt,
    observability_receipt: &psionic_train::PsionPretrainRunObservabilityReceipt,
) -> Result<psionic_train::PsionCheckpointRecoveryBundle, Box<dyn Error>> {
    let scope = CheckpointScopeBinding::new(CheckpointScopeKind::Run, stage_receipt.run_id.clone());
    let checkpoint_family = stage_receipt
        .checkpoint_lineage
        .promoted_checkpoint
        .checkpoint_family
        .clone();
    let base_checkpoint = stage_receipt.checkpoint_lineage.promoted_checkpoint.clone();

    let dense_manifest = CheckpointManifest::new(
        scope.clone(),
        checkpoint_family.clone(),
        base_checkpoint.clone(),
        vec![CheckpointShardManifest {
            shard_id: String::from("trusted-dense-shard-0"),
            manifest: checkpoint_stream_ref(
                "stream-psion-broad-pretrain-final-dense-v1",
                "manifest-psion-broad-pretrain-final-dense-v1",
                "object-psion-broad-pretrain-final-dense-v1",
                checkpoint_family.as_str(),
                base_checkpoint
                    .checkpoint_ref
                    .as_deref()
                    .unwrap_or("checkpoint://psion/broad/pretrain/final"),
                16_384,
                386_547_056,
            ),
            writer_node_id: String::from("node-psion-b"),
        }],
        CheckpointDurabilityPosture::Durable,
        1_742_620_900_000,
    )?;
    let dense_pointer = CheckpointPointer::new(
        scope.clone(),
        checkpoint_family.clone(),
        base_checkpoint.clone(),
        dense_manifest.manifest_digest.clone(),
        1_742_620_900_600,
    )?;

    let sharded_checkpoint = TrainingCheckpointReference::new(
        checkpoint_family.clone(),
        "stream-psion-broad-pretrain-final-sharded-v1",
        "manifest-psion-broad-pretrain-final-sharded-v1",
        "object-psion-broad-pretrain-final-sharded-v1",
        "node-psion-b",
        base_checkpoint.membership_epoch,
        base_checkpoint.cluster_state_digest.clone(),
        base_checkpoint.topology_digest.clone(),
        base_checkpoint.started_at_ms,
    )
    .with_checkpoint_ref("checkpoint://psion/broad/pretrain/final/sharded")
    .with_step(base_checkpoint.step.unwrap_or(16_384))
    .with_durable_at_ms(base_checkpoint.durable_at_ms.unwrap_or(1_742_620_900_000));
    let sharded_manifest = CheckpointManifest::new(
        scope.clone(),
        checkpoint_family.clone(),
        sharded_checkpoint.clone(),
        vec![
            checkpoint_shard_manifest(
                "trusted-shard-0",
                "stream-psion-broad-pretrain-final-shard-0-v1",
                "manifest-psion-broad-pretrain-final-shard-0-v1",
                "object-psion-broad-pretrain-final-shard-0-v1",
                checkpoint_family.as_str(),
                "checkpoint://psion/broad/pretrain/final/sharded",
                16_384,
                386_545_664,
                "node-psion-a",
            ),
            checkpoint_shard_manifest(
                "trusted-shard-1",
                "stream-psion-broad-pretrain-final-shard-1-v1",
                "manifest-psion-broad-pretrain-final-shard-1-v1",
                "object-psion-broad-pretrain-final-shard-1-v1",
                checkpoint_family.as_str(),
                "checkpoint://psion/broad/pretrain/final/sharded",
                16_384,
                386_545_664,
                "node-psion-b",
            ),
            checkpoint_shard_manifest(
                "trusted-shard-2",
                "stream-psion-broad-pretrain-final-shard-2-v1",
                "manifest-psion-broad-pretrain-final-shard-2-v1",
                "object-psion-broad-pretrain-final-shard-2-v1",
                checkpoint_family.as_str(),
                "checkpoint://psion/broad/pretrain/final/sharded",
                16_384,
                386_545_664,
                "node-psion-c",
            ),
            checkpoint_shard_manifest(
                "trusted-shard-3",
                "stream-psion-broad-pretrain-final-shard-3-v1",
                "manifest-psion-broad-pretrain-final-shard-3-v1",
                "object-psion-broad-pretrain-final-shard-3-v1",
                checkpoint_family.as_str(),
                "checkpoint://psion/broad/pretrain/final/sharded",
                16_384,
                386_545_664,
                "node-psion-d",
            ),
        ],
        CheckpointDurabilityPosture::Durable,
        1_742_620_920_000,
    )?;
    let sharded_pointer = CheckpointPointer::new(
        scope.clone(),
        checkpoint_family.clone(),
        sharded_checkpoint.clone(),
        sharded_manifest.manifest_digest.clone(),
        1_742_620_920_600,
    )?;

    let dense_artifact = record_psion_checkpoint_artifact(
        "psion-trusted-cluster-dense-checkpoint-artifact-v1",
        PsionCheckpointLayoutKind::Dense,
        stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint_label
            .clone(),
        stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint
            .object_digest
            .clone(),
        dense_manifest.clone(),
        dense_pointer.clone(),
        checkpoint_context(stage_receipt, observability_receipt),
        dense_optimizer_restart(),
        "Dense checkpoint artifact preserves one pointer-first last-stable fallback for the trusted-cluster lane.",
        stage_receipt,
        observability_receipt,
    )?;
    let sharded_artifact = record_psion_checkpoint_artifact(
        "psion-trusted-cluster-sharded-checkpoint-artifact-v1",
        PsionCheckpointLayoutKind::Sharded,
        stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint_label
            .clone(),
        stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint
            .object_digest
            .clone(),
        sharded_manifest.clone(),
        sharded_pointer.clone(),
        checkpoint_context(stage_receipt, observability_receipt),
        sharded_optimizer_restart(),
        "Sharded checkpoint artifact freezes the supported four-worker trusted-cluster restart path.",
        stage_receipt,
        observability_receipt,
    )?;

    let dense_restore = restore_receipt(
        dense_manifest.clone(),
        dense_pointer.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        &[NodeId::new("node-psion-b")],
    )?;
    let sharded_restore = restore_receipt(
        sharded_manifest.clone(),
        sharded_pointer.clone(),
        TrainingRecoveryMode::BlockingCatchUp,
        &[
            NodeId::new("node-psion-a"),
            NodeId::new("node-psion-b"),
            NodeId::new("node-psion-c"),
            NodeId::new("node-psion-d"),
        ],
    )?;
    let rollback_restore = stale_pointer_fallback_restore(
        dense_manifest.clone(),
        sharded_pointer.clone(),
        checkpoint_family.as_str(),
        scope.clone(),
    )?;

    let artifacts = vec![dense_artifact.clone(), sharded_artifact.clone()];
    let forced_restart = record_psion_checkpoint_recovery_event(
        "psion-trusted-cluster-forced-restart-v1",
        PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart,
        dense_artifact.artifact_id.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        Some(dense_restore),
        "psion-trusted-cluster-recovery-topology-single-device-v1",
        1,
        stage_receipt.dataset_identity.clone(),
        stage_receipt.sampling_policy_id.clone(),
        stage_receipt.sampling_policy_version.clone(),
        16_384,
        TrainingInstabilityTelemetry::default().with_checkpoint_catchup_latency_ms(260),
        continue_verdict(260, 0, 0),
        None,
        PsionCheckpointRecoveryDisposition::Resumed,
        None,
        "Forced interruption restarted from the trusted-cluster dense fallback artifact.",
        &artifacts,
    )?;
    let distributed_restart = record_psion_checkpoint_recovery_event(
        "psion-trusted-cluster-distributed-restart-v1",
        PsionCheckpointRecoveryEventKind::DistributedRestart,
        sharded_artifact.artifact_id.clone(),
        TrainingRecoveryMode::BlockingCatchUp,
        Some(sharded_restore),
        stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint
            .topology_digest
            .clone(),
        4,
        stage_receipt.dataset_identity.clone(),
        stage_receipt.sampling_policy_id.clone(),
        stage_receipt.sampling_policy_version.clone(),
        16_384,
        TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(430)
            .with_topology_churn_events(1),
        continue_verdict(430, 1, 0),
        None,
        PsionCheckpointRecoveryDisposition::Resumed,
        None,
        "Distributed restart resumed the trusted-cluster sharded checkpoint on the bounded four-worker topology.",
        &artifacts,
    )?;
    let rollback_corruption = record_psion_checkpoint_corruption(
        "psion-trusted-cluster-sharded-corruption-v1",
        sharded_artifact.artifact_id.clone(),
        sharded_artifact
            .checkpoint_manifest
            .manifest_digest
            .clone(),
        PsionCheckpointCorruptionKind::ManifestDigestMismatch,
        "Sharded trusted-cluster checkpoint corruption blocked continuation and forced rollback to the dense last-stable artifact.",
        &sharded_artifact,
    )?;
    let corruption_rollback = record_psion_checkpoint_recovery_event(
        "psion-trusted-cluster-corruption-rollback-v1",
        PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback,
        sharded_artifact.artifact_id.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        Some(rollback_restore),
        "psion-trusted-cluster-recovery-topology-rollback-v1",
        1,
        stage_receipt.dataset_identity.clone(),
        stage_receipt.sampling_policy_id.clone(),
        stage_receipt.sampling_policy_version.clone(),
        16_384,
        TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(810)
            .with_topology_churn_events(2),
        quarantine_verdict(810, 2),
        Some(rollback_corruption),
        PsionCheckpointRecoveryDisposition::RolledBackToStableCheckpoint,
        Some(dense_artifact.artifact_id.clone()),
        "Manifest corruption triggered rollback to the trusted-cluster dense last-stable artifact.",
        &artifacts,
    )?;
    let invalidation_corruption = record_psion_checkpoint_corruption(
        "psion-trusted-cluster-dense-optimizer-corruption-v1",
        dense_artifact.artifact_id.clone(),
        dense_artifact
            .optimizer_state_restart
            .optimizer_state_artifacts[0]
            .manifest_digest
            .clone(),
        PsionCheckpointCorruptionKind::OptimizerStateMismatch,
        "Optimizer-state corruption invalidated the trusted-cluster run instead of resuming from a poisoned state.",
        &dense_artifact,
    )?;
    let corruption_invalidation = record_psion_checkpoint_recovery_event(
        "psion-trusted-cluster-corruption-invalidation-v1",
        PsionCheckpointRecoveryEventKind::CorruptionDetectedInvalidation,
        dense_artifact.artifact_id.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        None,
        "psion-trusted-cluster-recovery-topology-invalidation-v1",
        1,
        stage_receipt.dataset_identity.clone(),
        stage_receipt.sampling_policy_id.clone(),
        stage_receipt.sampling_policy_version.clone(),
        16_384,
        TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(940)
            .with_environment_failure_rate_bps(140),
        halt_verdict(940, 140),
        Some(invalidation_corruption),
        PsionCheckpointRecoveryDisposition::Invalidated,
        None,
        "Optimizer-state corruption invalidated the trusted-cluster run instead of continuing.",
        &artifacts,
    )?;

    Ok(record_psion_checkpoint_recovery_bundle(
        "psion-trusted-cluster-checkpoint-recovery-bundle-v1",
        artifacts,
        vec![
            forced_restart,
            distributed_restart,
            corruption_rollback,
            corruption_invalidation,
        ],
        dense_artifact.artifact_id.clone(),
        "Trusted-cluster checkpoint recovery bundle freezes dense fallback, distributed restart, corruption rollback, and invalidation on the broader four-worker run.",
        stage_receipt,
        observability_receipt,
    )?)
}

fn checkpoint_context(
    stage_receipt: &PsionPretrainStageRunReceipt,
    observability_receipt: &psionic_train::PsionPretrainRunObservabilityReceipt,
) -> PsionCheckpointContextReceipt {
    PsionCheckpointContextReceipt {
        training_run_profile: observability_receipt.run_profile,
        dataset_identity: stage_receipt.dataset_identity.clone(),
        sampling_policy_id: stage_receipt.sampling_policy_id.clone(),
        sampling_policy_version: stage_receipt.sampling_policy_version.clone(),
        source_checkpoint_topology_digest: stage_receipt
            .checkpoint_lineage
            .promoted_checkpoint
            .topology_digest
            .clone(),
        training_hardware_topology_digest: observability_receipt
            .hardware_topology
            .topology_digest
            .clone(),
        observed_worker_count: observability_receipt.hardware_topology.observed_worker_count,
        detail: String::from(
            "Trusted-cluster checkpoint artifacts preserve the source checkpoint topology and realized multi-host hardware topology separately.",
        ),
    }
}

fn dense_optimizer_restart() -> psionic_train::PsionOptimizerStateRestartReceipt {
    psionic_train::PsionOptimizerStateRestartReceipt {
        optimizer_family: String::from("adamw"),
        optimizer_checkpoint_family: String::from("train.psion.decoder.optimizer_state"),
        optimizer_state_step: 16_384,
        parameter_group_count: 16,
        optimizer_state_artifacts: vec![checkpoint_stream_ref(
            "stream-psion-broad-pretrain-final-optimizer-v1",
            "manifest-psion-broad-pretrain-final-optimizer-v1",
            "object-psion-broad-pretrain-final-optimizer-v1",
            "train.psion.decoder.optimizer_state",
            "checkpoint://psion/broad/pretrain/final/optimizer_state",
            16_384,
            773_091_328,
        )],
        strict_parameter_group_order_restore: true,
        resume_requires_matching_sampling_cursor: true,
        summary: String::from(
            "Dense optimizer-state restart preserves exact parameter-group order and sampling cursor binding for the broader run.",
        ),
    }
}

fn sharded_optimizer_restart() -> psionic_train::PsionOptimizerStateRestartReceipt {
    psionic_train::PsionOptimizerStateRestartReceipt {
        optimizer_family: String::from("adamw"),
        optimizer_checkpoint_family: String::from("train.psion.decoder.optimizer_state"),
        optimizer_state_step: 16_384,
        parameter_group_count: 16,
        optimizer_state_artifacts: vec![
            checkpoint_stream_ref(
                "stream-psion-broad-pretrain-final-optimizer-shard-0-v1",
                "manifest-psion-broad-pretrain-final-optimizer-shard-0-v1",
                "object-psion-broad-pretrain-final-optimizer-shard-0-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/broad/pretrain/final/optimizer_state/sharded",
                16_384,
                193_272_832,
            ),
            checkpoint_stream_ref(
                "stream-psion-broad-pretrain-final-optimizer-shard-1-v1",
                "manifest-psion-broad-pretrain-final-optimizer-shard-1-v1",
                "object-psion-broad-pretrain-final-optimizer-shard-1-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/broad/pretrain/final/optimizer_state/sharded",
                16_384,
                193_272_832,
            ),
            checkpoint_stream_ref(
                "stream-psion-broad-pretrain-final-optimizer-shard-2-v1",
                "manifest-psion-broad-pretrain-final-optimizer-shard-2-v1",
                "object-psion-broad-pretrain-final-optimizer-shard-2-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/broad/pretrain/final/optimizer_state/sharded",
                16_384,
                193_272_832,
            ),
            checkpoint_stream_ref(
                "stream-psion-broad-pretrain-final-optimizer-shard-3-v1",
                "manifest-psion-broad-pretrain-final-optimizer-shard-3-v1",
                "object-psion-broad-pretrain-final-optimizer-shard-3-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/broad/pretrain/final/optimizer_state/sharded",
                16_384,
                193_272_832,
            ),
        ],
        strict_parameter_group_order_restore: true,
        resume_requires_matching_sampling_cursor: true,
        summary: String::from(
            "Sharded optimizer-state restart preserves group order, step identity, and sampling cursor binding across four trusted-cluster shards.",
        ),
    }
}

fn checkpoint_stream_ref(
    stream_id: &str,
    manifest_digest: &str,
    object_digest: &str,
    checkpoint_family: &str,
    checkpoint_ref: &str,
    step: u64,
    total_bytes: u64,
) -> DatastreamManifestRef {
    DatastreamManifestRef {
        stream_id: String::from(stream_id),
        manifest_digest: String::from(manifest_digest),
        subject: DatastreamSubjectKind::Checkpoint,
        object_digest: String::from(object_digest),
        total_bytes,
        chunk_count: 8,
        chunk_bytes: 4 * 1024 * 1024,
        encoding: DatastreamEncoding::Safetensors,
        compression: None,
        provenance_digest: None,
        dataset_binding: None,
        checkpoint_binding: Some(
            DatastreamCheckpointBinding::new(checkpoint_family)
                .with_checkpoint_ref(checkpoint_ref)
                .with_step(step),
        ),
        policy_weight_binding: None,
        mirrors: Vec::new(),
    }
}

fn checkpoint_shard_manifest(
    shard_id: &str,
    stream_id: &str,
    manifest_digest: &str,
    object_digest: &str,
    checkpoint_family: &str,
    checkpoint_ref: &str,
    step: u64,
    total_bytes: u64,
    writer_node_id: &str,
) -> CheckpointShardManifest {
    CheckpointShardManifest {
        shard_id: String::from(shard_id),
        manifest: checkpoint_stream_ref(
            stream_id,
            manifest_digest,
            object_digest,
            checkpoint_family,
            checkpoint_ref,
            step,
            total_bytes,
        ),
        writer_node_id: String::from(writer_node_id),
    }
}

fn restore_receipt(
    manifest: CheckpointManifest,
    pointer: CheckpointPointer,
    recovery_mode: TrainingRecoveryMode,
    uploader_candidates: &[NodeId],
) -> Result<psionic_train::CheckpointRestoreReceipt, Box<dyn Error>> {
    let mut store = InMemoryCheckpointStore::default();
    store.store_manifest(manifest.clone());
    store.store_pointer(pointer);
    Ok(store.plan_restore(
        &manifest.scope,
        manifest.checkpoint_family.as_str(),
        recovery_mode,
        uploader_candidates,
        CheckpointStoreReadOptions::default(),
    )?)
}

fn stale_pointer_fallback_restore(
    dense_manifest: CheckpointManifest,
    stale_pointer: CheckpointPointer,
    checkpoint_family: &str,
    scope: CheckpointScopeBinding,
) -> Result<psionic_train::CheckpointRestoreReceipt, Box<dyn Error>> {
    let mut store = InMemoryCheckpointStore::default();
    store.store_manifest(dense_manifest);
    store.store_pointer(stale_pointer);
    Ok(store.plan_restore(
        &scope,
        checkpoint_family,
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        &[NodeId::new("node-psion-b")],
        CheckpointStoreReadOptions::default(),
    )?)
}

fn continue_verdict(
    checkpoint_catchup_latency_ms: u64,
    topology_churn_events: u32,
    environment_failure_rate_bps: u32,
) -> TrainingStabilityVerdict {
    stability_controller().evaluate(
        &TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(checkpoint_catchup_latency_ms)
            .with_topology_churn_events(topology_churn_events)
            .with_environment_failure_rate_bps(environment_failure_rate_bps),
        &[],
    )
}

fn quarantine_verdict(
    checkpoint_catchup_latency_ms: u64,
    topology_churn_events: u32,
) -> TrainingStabilityVerdict {
    stability_controller().evaluate(
        &TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(checkpoint_catchup_latency_ms)
            .with_topology_churn_events(topology_churn_events),
        &[],
    )
}

fn halt_verdict(
    checkpoint_catchup_latency_ms: u64,
    environment_failure_rate_bps: u32,
) -> TrainingStabilityVerdict {
    stability_controller().evaluate(
        &TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(checkpoint_catchup_latency_ms)
            .with_environment_failure_rate_bps(environment_failure_rate_bps),
        &[],
    )
}

fn stability_controller() -> TrainingStabilityController {
    TrainingStabilityController::new(TrainingInstabilityPolicy::new(
        vec![
            TrainingInstabilityRule {
                signal: TrainingInstabilitySignalKind::CheckpointCatchupLatencyMs,
                max_value: 500.0,
                action: TrainingOperationalAction::Quarantine,
            },
            TrainingInstabilityRule {
                signal: TrainingInstabilitySignalKind::EnvironmentFailureRateBps,
                max_value: 100.0,
                action: TrainingOperationalAction::Halt,
            },
        ],
        vec![TrainingRiskyOptimizationRule {
            optimization: TrainingRiskyOptimization::AsyncCheckpointOverlap,
            action: TrainingOperationalAction::Quarantine,
        }],
    ))
}

fn trusted_cluster_replay_pair(
) -> Result<(TrainingReplayReceipt, TrainingReplayReceipt), Box<dyn Error>> {
    let package = sample_environment_package();
    let batch = sample_trainer_batch(&package.key)?;
    let seed_discipline = TrainingReplaySeedDiscipline::new(404, 505, 606);
    let environment_pin =
        ReplayEnvironmentPin::from_package(&package, &sample_tool_versions("planner@1"))?;
    let eval_posture = ReproducibleEvalPosture::from_eval_contract(
        &sample_eval_contract(&package.key),
        deterministic_eval_strategy(),
        seed_discipline.eval_seed,
        vec![
            String::from("sample-a"),
            String::from("sample-b"),
            String::from("sample-c"),
            String::from("sample-d"),
        ],
    )?;
    let sample_selection_rules = vec![
        DeterministicSampleSelectionRule::derive(
            seed_discipline.assignment_seed,
            "assignment-a",
            "worker-a",
            0,
            vec![String::from("task-a"), String::from("task-b")],
            vec![String::from("task-a")],
        )?,
        DeterministicSampleSelectionRule::derive(
            seed_discipline.assignment_seed,
            "assignment-b",
            "worker-b",
            0,
            vec![String::from("task-c"), String::from("task-d")],
            vec![String::from("task-c")],
        )?,
        DeterministicSampleSelectionRule::derive(
            seed_discipline.assignment_seed,
            "assignment-c",
            "worker-c",
            0,
            vec![String::from("task-e"), String::from("task-f")],
            vec![String::from("task-e")],
        )?,
        DeterministicSampleSelectionRule::derive(
            seed_discipline.assignment_seed,
            "assignment-d",
            "worker-d",
            0,
            vec![String::from("task-g"), String::from("task-h")],
            vec![String::from("task-g")],
        )?,
    ];
    let baseline = TrainingReplayReceipt::new(
        batch.clone(),
        seed_discipline,
        sample_selection_rules.clone(),
        environment_pin.clone(),
        eval_posture.clone(),
    )?;
    let observed = TrainingReplayReceipt::new(
        batch,
        seed_discipline,
        sample_selection_rules,
        environment_pin,
        eval_posture,
    )?;
    Ok((baseline, observed))
}

fn sample_environment_package() -> EnvironmentPackageContract {
    EnvironmentPackageContract::new(
        EnvironmentPackageKey::new("psion.cluster.reasoner", "1.0.0"),
        EnvironmentPackageFamily::Agentic,
        "Psion Cluster Reasoner",
        EnvironmentExecutionEntrypoint {
            runtime_family: EnvironmentRuntimeFamily::MultiTurnDialog,
            entrypoint: String::from("psion.cluster.run"),
            args: vec![String::from("--trusted-cluster")],
            sandbox_profile_ref: None,
            max_turns: 4,
            state_mode: EnvironmentStateMode::SessionPersistent,
            time_budget_ms: Some(8_000),
        },
    )
    .with_tools(vec![EnvironmentToolContract {
        tool_name: String::from("inspect_topology"),
        interface: EnvironmentToolInterface::NativeFunction,
        description: String::from("Inspect one trusted-cluster topology"),
        args_schema: json!({"type": "object", "required": ["cluster_id"]}),
        result_schema: Some(json!({"type": "object"})),
    }])
}

fn sample_eval_contract(environment: &EnvironmentPackageKey) -> EvalRunContract {
    EvalRunContract::new(
        "eval-psion-trusted-cluster-v1",
        EvalRunMode::OfflineHeldOut,
        environment.clone(),
    )
    .with_expected_sample_count(4)
}

fn deterministic_eval_strategy() -> EvalExecutionStrategyFacts {
    EvalExecutionStrategyFacts {
        strategy_label: String::from("validator"),
        runtime_family: Some(String::from("sandbox")),
        scheduler_posture: Some(String::from("deterministic")),
    }
}

fn sample_tool_versions(version: &str) -> BTreeMap<String, String> {
    BTreeMap::from([(String::from("inspect_topology"), String::from(version))])
}

fn sample_trainer_batch(
    environment: &EnvironmentPackageKey,
) -> Result<TrainerBatch, Box<dyn Error>> {
    let target_revision =
        PolicyRevision::new("psion.policy", "rev-3", "psion-target-policy-digest", 3_000)
            .with_revision_number(3);
    let source_revision =
        PolicyRevision::new("psion.policy", "rev-2", "psion-source-policy-digest", 2_000)
            .with_revision_number(2);
    let rollout_a = RolloutArtifact::new(
        "rollout-a",
        "worker-a",
        environment.clone(),
        "task-a",
        source_revision.clone(),
        vec![RolloutSample::new(1, -0.2, 0.8, 0.6)],
        RolloutTerminationReason::Completed,
        vec![RolloutProofReference::new(
            RolloutProofKind::ExecutionProof,
            "proof-a",
            "proof://a",
        )],
        3_000,
    )?;
    let rollout_b = RolloutArtifact::new(
        "rollout-b",
        "worker-b",
        environment.clone(),
        "task-c",
        source_revision.clone(),
        vec![RolloutSample::new(2, -0.25, 0.7, 0.5)],
        RolloutTerminationReason::Completed,
        vec![RolloutProofReference::new(
            RolloutProofKind::ExecutionProof,
            "proof-b",
            "proof://b",
        )],
        3_050,
    )?;
    let rollout_c = RolloutArtifact::new(
        "rollout-c",
        "worker-c",
        environment.clone(),
        "task-e",
        source_revision.clone(),
        vec![RolloutSample::new(3, -0.18, 0.82, 0.63)],
        RolloutTerminationReason::Completed,
        vec![RolloutProofReference::new(
            RolloutProofKind::ExecutionProof,
            "proof-c",
            "proof://c",
        )],
        3_100,
    )?;
    let rollout_d = RolloutArtifact::new(
        "rollout-d",
        "worker-d",
        environment.clone(),
        "task-g",
        source_revision,
        vec![RolloutSample::new(4, -0.22, 0.76, 0.59)],
        RolloutTerminationReason::Completed,
        vec![RolloutProofReference::new(
            RolloutProofKind::ExecutionProof,
            "proof-d",
            "proof://d",
        )],
        3_150,
    )?;
    Ok(TrainerBatch::assemble(
        "trainer-batch-psion-trusted-cluster-v1",
        target_revision,
        vec![rollout_a, rollout_b, rollout_c, rollout_d],
        4_000,
    )?)
}

fn broader_run_observability(
    stage_receipt: &PsionPretrainStageRunReceipt,
) -> Result<psionic_train::PsionPretrainRunObservabilityReceipt, Box<dyn Error>> {
    let devices = vec![
        device(
            "cuda:h100-0",
            "0000:81:00.0",
            80 * 1024 * 1024 * 1024,
            63 * 1024 * 1024 * 1024,
        ),
        device(
            "cuda:h100-1",
            "0000:82:00.0",
            80 * 1024 * 1024 * 1024,
            61 * 1024 * 1024 * 1024,
        ),
        device(
            "cuda:h100-2",
            "0000:83:00.0",
            80 * 1024 * 1024 * 1024,
            60 * 1024 * 1024 * 1024,
        ),
        device(
            "cuda:h100-3",
            "0000:84:00.0",
            80 * 1024 * 1024 * 1024,
            59 * 1024 * 1024 * 1024,
        ),
    ];
    let topology = ExecutionTopologyPlan::tensor_sharded(
        "cuda",
        0,
        vec![
            (devices[0].clone(), 0, 256),
            (devices[1].clone(), 256, 512),
            (devices[2].clone(), 512, 768),
            (devices[3].clone(), 768, 1024),
        ],
    );
    let membership = broader_membership();
    let training_recovery = TrainingRecoveryContext::new(
        TrainingRecoveryPosture::ElasticReconfiguration,
        TrainingCheckpointAvailability::Durable,
        membership.clone(),
    )
    .with_latest_checkpoint(stage_receipt.checkpoint_lineage.promoted_checkpoint.clone())
    .with_recovering_node_ids(vec![String::from("worker-d")])
    .with_requested_at_ms(1_742_620_500_000)
    .with_detail("One worker rejoined after a short topology churn event during the broader run.");
    let collective = TrainingCollectiveContext::new(
        TrainingDeviceMeshContext::new(
            "psion-broad-mesh",
            7,
            "cuda",
            ClusterCommunicationClass::TensorCollectiveMesh,
            membership,
            node_ids(),
        ),
        TrainingCollectiveKind::AllReduce,
        TrainingCollectiveQuantization::Int8Symmetric,
        512 * 1024 * 1024,
        192 * 1024 * 1024,
        4,
    )
    .with_benchmark("psion-broad-collective-benchmark-v1", 1670, 12)
    .with_detail(
        "Tensor-parallel gradient reductions stayed on the justified int8 collective lane.",
    );
    let cluster_execution = ClusterExecutionContext::new(
        "cluster-psion-trusted-a",
        "cluster-state-digest-psion-broad-v1",
        "topology-digest-psion-broad-v1",
        "scheduler-psion-a",
        ClusterTransportClass::TrustedLanStream,
        ClusterExecutionDisposition::Sharded,
    )
    .with_execution_topology(topology.clone())
    .with_selected_nodes(vec![
        ClusterSelectedNode::new("worker-a", "cuda").with_device_inventory(devices[0].clone()),
        ClusterSelectedNode::new("worker-b", "cuda").with_device_inventory(devices[1].clone()),
        ClusterSelectedNode::new("worker-c", "cuda").with_device_inventory(devices[2].clone()),
        ClusterSelectedNode::new("worker-d", "cuda").with_device_inventory(devices[3].clone()),
    ])
    .with_training_recovery(training_recovery)
    .with_training_collective(collective);
    let telemetry = TrainingInstabilityTelemetry::default()
        .with_entropy_drift_bps(180)
        .with_checkpoint_catchup_latency_ms(2400)
        .with_topology_churn_events(2)
        .with_environment_failure_rate_bps(120)
        .with_sandbox_failure_rate_bps(45);
    let stability_verdict = TrainingStabilityController::new(TrainingInstabilityPolicy::new(
        vec![
            TrainingInstabilityRule {
                signal: TrainingInstabilitySignalKind::EntropyDriftBps,
                max_value: 100.0,
                action: TrainingOperationalAction::Continue,
            },
            TrainingInstabilityRule {
                signal: TrainingInstabilitySignalKind::CheckpointCatchupLatencyMs,
                max_value: 1500.0,
                action: TrainingOperationalAction::Quarantine,
            },
            TrainingInstabilityRule {
                signal: TrainingInstabilitySignalKind::TopologyChurnEvents,
                max_value: 0.0,
                action: TrainingOperationalAction::Continue,
            },
        ],
        Vec::new(),
    ))
    .evaluate(&telemetry, &[]);
    Ok(record_psion_pretrain_run_observability(
        "psion-broader-pretrain-observability-v1",
        PsionPretrainRunScaleProfile::BroaderPretraining,
        PsionPretrainRunCostReceipt {
            cost_basis: PsionPretrainRunCostBasis::MeteredUsd,
            currency_code: String::from("USD"),
            compute_cost_microusd: 487_250_000,
            storage_cost_microusd: 18_600_000,
            network_cost_microusd: 6_400_000,
            total_cost_microusd: 512_250_000,
            detail: String::from(
                "Broader run cost reflects metered trusted-cluster accelerator time plus checkpoint storage and east-west traffic.",
            ),
        },
        PsionPretrainRunThroughputReceipt {
            train_tokens_processed: 1_073_741_824,
            validation_tokens_processed: 33_554_432,
            held_out_tokens_scored: 8_388_608,
            optimizer_steps_completed: 16_384,
            wall_clock_ms: 3_780_000,
            mean_tokens_per_second: 296_214,
            peak_tokens_per_second: 331_442,
            mean_sequences_per_second_milli: 72_500,
            mean_step_latency_ms: 231,
            checkpoint_write_throughput_bytes_per_second: 1_476_395_008,
        },
        PsionPretrainCheckpointArtifactReceipt {
            promoted_checkpoint_label: stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .clone(),
            checkpoint_family: stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .checkpoint_family
                .clone(),
            checkpoint_object_digest: stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .object_digest
                .clone(),
            checkpoint_size_bytes: 1_546_182_656,
            optimizer_state_size_bytes: 773_091_328,
            ancillary_artifact_size_bytes: 14_680_064,
            total_artifact_size_bytes: 2_333_954_048,
            shard_count: 8,
            detail: String::from(
                "Broader run artifact surface includes sharded weights, optimizer state, and receipt/descriptor sidecars.",
            ),
        },
        PsionPretrainHardwareTopologyReceipt::new(
            4,
            DeliveredExecutionContext::new("cuda", Some(topology), devices)
                .with_cluster_execution(cluster_execution),
            "Broader pretraining run preserved explicit tensor-sharded cluster topology and recovery facts.",
        )?,
        telemetry,
        Some(stability_verdict),
        "Broader pretraining observability receipt records scale-up throughput, metered cost, checkpoint size, cluster topology, and structured instability markers.",
        stage_receipt,
    )?)
}

fn broader_stage_receipt(root: &PathBuf) -> Result<PsionPretrainStageRunReceipt, Box<dyn Error>> {
    let model_descriptor: PsionCompactDecoderDescriptor =
        serde_json::from_str(&fs::read_to_string(root.join(
            "fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json",
        ))?)?;
    let tokenized_corpus: PsionTokenizedCorpusManifest =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"),
        )?)?;
    let sampling_policy: PsionSamplingPolicyManifest = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json"),
    )?)?;
    let stage_config = PsionPretrainStageConfig::new(
        "run-psion-broad",
        "run-psion-broad-stage-1-pretrain",
        PsionPretrainObjectiveConfig {
            objective_kind: PsionPretrainObjectiveKind::NextTokenPrediction,
            loss_normalization: PsionPretrainLossNormalization::ByTargetToken,
            label_smoothing_bps: 20,
            tokenizer_binding_digest: model_descriptor.tokenizer_binding.stable_digest(),
            dataset_identity: tokenized_corpus
                .replay_contract
                .stable_dataset_identity
                .clone(),
            max_context_tokens: model_descriptor.config.max_context,
        },
        &model_descriptor,
        &tokenized_corpus,
        &sampling_policy,
    )?;
    let replay_receipt = PsionPretrainReplayReceipt::new(
        "psion-broad-pretrain-replay-v1",
        tokenized_corpus.replay_contract.stable_dataset_identity.clone(),
        tokenized_corpus.replay_contract.iteration_mode,
        tokenized_corpus.replay_contract.shard_ordering,
        tokenized_corpus.replay_contract.deterministic_shuffle_seed,
        3,
        true,
        "Broader run replay checks matched the tokenized-corpus contract across three recovery rehearsals.",
    );
    let checkpoint_lineage = PsionPretrainCheckpointLineageReceipt::new(
        "psion-broad-pretrain-checkpoint-lineage-v1",
        TrainingCheckpointReference::new(
            "train.psion.decoder",
            "stream-psion-broad-pretrain-final-v1",
            "manifest-psion-broad-pretrain-final-v1",
            "object-psion-broad-pretrain-final-v1",
            "node-psion-b",
            7,
            "cluster-state-digest-psion-broad-v1",
            "topology-digest-psion-broad-v1",
            1_742_620_000_000,
        )
        .with_checkpoint_ref("checkpoint://psion/broad/pretrain/final")
        .with_step(16_384)
        .with_durable_at_ms(1_742_620_900_000),
        None,
        "broader-pretrain-final",
        model_descriptor.model.model_id.clone(),
        model_descriptor.stable_digest(),
    );
    Ok(run_psion_pretrain_stage(
        &stage_config,
        vec![
            PsionPretrainSourceFamilyReportRow {
                split_name: String::from("held_out"),
                split_kind: DatasetSplitKind::HeldOut,
                source_family_id: String::from("evaluation_only_benchmark_material"),
                source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
                token_share_bps_within_split: 10_000,
                sequence_share_bps_within_split: 10_000,
                mean_next_token_loss_milli: 1210,
                detail: String::from(
                    "Held-out benchmark material remains isolated for broader pretraining evaluation.",
                ),
            },
            PsionPretrainSourceFamilyReportRow {
                split_name: String::from("train"),
                split_kind: DatasetSplitKind::Train,
                source_family_id: String::from("computer_architecture_history"),
                source_ids: vec![String::from("arch_textbook_foster_1985")],
                token_share_bps_within_split: 5550,
                sequence_share_bps_within_split: 5450,
                mean_next_token_loss_milli: 980,
                detail: String::from(
                    "Broader run keeps prose slightly ahead while reducing train loss materially.",
                ),
            },
            PsionPretrainSourceFamilyReportRow {
                split_name: String::from("train"),
                split_kind: DatasetSplitKind::Train,
                source_family_id: String::from("normative_specs"),
                source_ids: vec![String::from("wasm_core_spec_release_2")],
                token_share_bps_within_split: 4450,
                sequence_share_bps_within_split: 4550,
                mean_next_token_loss_milli: 1035,
                detail: String::from(
                    "Broader run preserves heavy spec coverage alongside the prose anchor.",
                ),
            },
            PsionPretrainSourceFamilyReportRow {
                split_name: String::from("validation"),
                split_kind: DatasetSplitKind::Validation,
                source_family_id: String::from("computer_architecture_history"),
                source_ids: vec![String::from("arch_textbook_foster_1985")],
                token_share_bps_within_split: 5200,
                sequence_share_bps_within_split: 5150,
                mean_next_token_loss_milli: 1015,
                detail: String::from(
                    "Validation prose stays slightly dominant for broader-run reasoning checks.",
                ),
            },
            PsionPretrainSourceFamilyReportRow {
                split_name: String::from("validation"),
                split_kind: DatasetSplitKind::Validation,
                source_family_id: String::from("normative_specs"),
                source_ids: vec![String::from("wasm_core_spec_release_2")],
                token_share_bps_within_split: 4800,
                sequence_share_bps_within_split: 4850,
                mean_next_token_loss_milli: 1080,
                detail: String::from(
                    "Validation spec coverage remains high so interpretation drift is visible.",
                ),
            },
        ],
        replay_receipt,
        checkpoint_lineage,
        "Broader Psion pretrain stage scales the explicit next-token lane onto the internal compact decoder while preserving replay and checkpoint lineage.",
        &model_descriptor,
        &tokenized_corpus,
        &sampling_policy,
    )?)
}

fn broader_membership() -> TrainingElasticMembershipContext {
    TrainingElasticMembershipContext::new(
        7,
        "cluster-state-digest-psion-broad-v1",
        "topology-digest-psion-broad-v1",
        node_ids(),
    )
}

fn node_ids() -> Vec<String> {
    vec![
        String::from("worker-a"),
        String::from("worker-b"),
        String::from("worker-c"),
        String::from("worker-d"),
    ]
}

fn device(
    stable_device_id: &str,
    topology_key: &str,
    total_memory_bytes: u64,
    free_memory_bytes: u64,
) -> DeviceInventoryQualifiers {
    DeviceInventoryQualifiers {
        stable_device_id: String::from(stable_device_id),
        topology_key: Some(String::from(topology_key)),
        performance_class: DevicePerformanceClass::DiscreteAccelerator,
        memory_class: DeviceMemoryClass::DedicatedDevice,
        total_memory_bytes: Some(total_memory_bytes),
        free_memory_bytes: Some(free_memory_bytes),
    }
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| String::from("workspace root should exist").into())
}
