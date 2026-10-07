use std::{error::Error, fs, path::PathBuf};

use psionic_cluster::NodeId;
use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifestRef, DatastreamSubjectKind,
};
use psionic_runtime::TrainingCheckpointReference;
use psionic_train::{
    record_psion_checkpoint_artifact, record_psion_checkpoint_corruption,
    record_psion_checkpoint_recovery_bundle, record_psion_checkpoint_recovery_event,
    CheckpointDurabilityPosture, CheckpointManifest, CheckpointPointer, CheckpointScopeBinding,
    CheckpointScopeKind, CheckpointShardManifest, CheckpointStoreReadOptions,
    InMemoryCheckpointStore, PsionCheckpointContextReceipt, PsionCheckpointCorruptionKind,
    PsionCheckpointLayoutKind, PsionCheckpointRecoveryDisposition,
    PsionCheckpointRecoveryEventKind, PsionOptimizerStateRestartReceipt,
    PsionPretrainRunObservabilityReceipt, PsionPretrainStageRunReceipt, TrainingInstabilityPolicy,
    TrainingInstabilityRule, TrainingInstabilityTelemetry, TrainingOperationalAction,
    TrainingRecoveryMode, TrainingRiskyOptimization, TrainingRiskyOptimizationRule,
    TrainingStabilityController, TrainingStabilityVerdict,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/checkpoint_recovery");
    fs::create_dir_all(&fixtures_dir)?;

    let stage_receipt: PsionPretrainStageRunReceipt = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"),
    )?)?;
    let observability_receipt: PsionPretrainRunObservabilityReceipt =
        serde_json::from_str(&fs::read_to_string(root.join(
            "fixtures/psion/observability/psion_pilot_pretrain_run_observability_receipt_v1.json",
        ))?)?;

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
            shard_id: String::from("dense-shard-0"),
            manifest: checkpoint_stream_ref(
                "stream-psion-pretrain-final-v1",
                "manifest-psion-pretrain-final-v1",
                "object-psion-pretrain-final-v1",
                checkpoint_family.as_str(),
                base_checkpoint
                    .checkpoint_ref
                    .as_deref()
                    .unwrap_or("checkpoint://psion/pilot/pretrain/final"),
                2048,
                143_654_912,
            ),
            writer_node_id: String::from("node-psion-a"),
        }],
        CheckpointDurabilityPosture::Durable,
        1_742_615_100_000,
    )?;
    let dense_pointer = CheckpointPointer::new(
        scope.clone(),
        checkpoint_family.clone(),
        base_checkpoint.clone(),
        dense_manifest.manifest_digest.clone(),
        1_742_615_100_500,
    )?;

    let sharded_checkpoint = TrainingCheckpointReference::new(
        checkpoint_family.clone(),
        "stream-psion-pretrain-final-sharded-v1",
        "manifest-psion-pretrain-final-sharded-v1",
        "object-psion-pretrain-final-sharded-v1",
        "node-psion-a",
        base_checkpoint.membership_epoch,
        base_checkpoint.cluster_state_digest.clone(),
        base_checkpoint.topology_digest.clone(),
        base_checkpoint.started_at_ms,
    )
    .with_checkpoint_ref("checkpoint://psion/pilot/pretrain/final/sharded")
    .with_step(base_checkpoint.step.unwrap_or(2048))
    .with_durable_at_ms(base_checkpoint.durable_at_ms.unwrap_or(1_742_615_100_000));
    let sharded_manifest = CheckpointManifest::new(
        scope.clone(),
        checkpoint_family.clone(),
        sharded_checkpoint.clone(),
        vec![
            checkpoint_shard_manifest(
                "sharded-shard-0",
                "stream-psion-pretrain-final-shard-0-v1",
                "manifest-psion-pretrain-final-shard-0-v1",
                "object-psion-pretrain-final-shard-0-v1",
                checkpoint_family.as_str(),
                "checkpoint://psion/pilot/pretrain/final/sharded",
                2048,
                36_000_000,
                "node-psion-a",
            ),
            checkpoint_shard_manifest(
                "sharded-shard-1",
                "stream-psion-pretrain-final-shard-1-v1",
                "manifest-psion-pretrain-final-shard-1-v1",
                "object-psion-pretrain-final-shard-1-v1",
                checkpoint_family.as_str(),
                "checkpoint://psion/pilot/pretrain/final/sharded",
                2048,
                36_000_000,
                "node-psion-b",
            ),
            checkpoint_shard_manifest(
                "sharded-shard-2",
                "stream-psion-pretrain-final-shard-2-v1",
                "manifest-psion-pretrain-final-shard-2-v1",
                "object-psion-pretrain-final-shard-2-v1",
                checkpoint_family.as_str(),
                "checkpoint://psion/pilot/pretrain/final/sharded",
                2048,
                36_000_000,
                "node-psion-c",
            ),
            checkpoint_shard_manifest(
                "sharded-shard-3",
                "stream-psion-pretrain-final-shard-3-v1",
                "manifest-psion-pretrain-final-shard-3-v1",
                "object-psion-pretrain-final-shard-3-v1",
                checkpoint_family.as_str(),
                "checkpoint://psion/pilot/pretrain/final/sharded",
                2048,
                36_000_000,
                "node-psion-d",
            ),
        ],
        CheckpointDurabilityPosture::Durable,
        1_742_615_120_000,
    )?;
    let sharded_pointer = CheckpointPointer::new(
        scope.clone(),
        checkpoint_family.clone(),
        sharded_checkpoint.clone(),
        sharded_manifest.manifest_digest.clone(),
        1_742_615_120_500,
    )?;

    let dense_artifact = record_psion_checkpoint_artifact(
        "psion-dense-checkpoint-artifact-v1",
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
        checkpoint_context(&stage_receipt, &observability_receipt),
        dense_optimizer_restart(),
        "Dense checkpoint artifact preserves exact pointer-first restart semantics for the promoted checkpoint.",
        &stage_receipt,
        &observability_receipt,
    )?;
    let sharded_artifact = record_psion_checkpoint_artifact(
        "psion-sharded-checkpoint-artifact-v1",
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
        checkpoint_context(&stage_receipt, &observability_receipt),
        sharded_optimizer_restart(),
        "Sharded checkpoint artifact freezes the distributed restart mirror over the same logical promoted checkpoint.",
        &stage_receipt,
        &observability_receipt,
    )?;

    let dense_restore = restore_receipt(
        dense_manifest.clone(),
        dense_pointer.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        &[NodeId::new("node-psion-a")],
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
        "psion-forced-interruption-restart-v1",
        PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart,
        dense_artifact.artifact_id.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        Some(dense_restore),
        "psion-recovery-topology-single-device-v1",
        1,
        stage_receipt.dataset_identity.clone(),
        stage_receipt.sampling_policy_id.clone(),
        stage_receipt.sampling_policy_version.clone(),
        2048,
        TrainingInstabilityTelemetry::default().with_checkpoint_catchup_latency_ms(220),
        continue_verdict(220, 0, 0),
        None,
        PsionCheckpointRecoveryDisposition::Resumed,
        None,
        "Forced interruption restarted from the dense checkpoint through pointer lookup.",
        &artifacts,
    )?;
    let distributed_restart = record_psion_checkpoint_recovery_event(
        "psion-distributed-restart-v1",
        PsionCheckpointRecoveryEventKind::DistributedRestart,
        sharded_artifact.artifact_id.clone(),
        TrainingRecoveryMode::BlockingCatchUp,
        Some(sharded_restore),
        "psion-recovery-topology-four-worker-v1",
        4,
        stage_receipt.dataset_identity.clone(),
        stage_receipt.sampling_policy_id.clone(),
        stage_receipt.sampling_policy_version.clone(),
        2048,
        TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(420)
            .with_topology_churn_events(1),
        continue_verdict(420, 1, 0),
        None,
        PsionCheckpointRecoveryDisposition::Resumed,
        None,
        "Distributed restart resumed the sharded mirror on a four-worker recovery topology.",
        &artifacts,
    )?;
    let rollback_corruption = record_psion_checkpoint_corruption(
        "psion-sharded-corruption-v1",
        sharded_artifact.artifact_id.clone(),
        sharded_artifact.checkpoint_manifest.manifest_digest.clone(),
        PsionCheckpointCorruptionKind::ManifestDigestMismatch,
        "Sharded checkpoint corruption blocked continuation and forced rollback to the last stable dense artifact.",
        &sharded_artifact,
    )?;
    let corruption_rollback = record_psion_checkpoint_recovery_event(
        "psion-corruption-rollback-v1",
        PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback,
        sharded_artifact.artifact_id.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        Some(rollback_restore),
        "psion-recovery-topology-rollback-v1",
        1,
        stage_receipt.dataset_identity.clone(),
        stage_receipt.sampling_policy_id.clone(),
        stage_receipt.sampling_policy_version.clone(),
        2048,
        TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(780)
            .with_topology_churn_events(2),
        quarantine_verdict(780, 2),
        Some(rollback_corruption),
        PsionCheckpointRecoveryDisposition::RolledBackToStableCheckpoint,
        Some(dense_artifact.artifact_id.clone()),
        "Manifest corruption triggered listing fallback and rollback to the last stable dense artifact.",
        &artifacts,
    )?;
    let invalidation_corruption = record_psion_checkpoint_corruption(
        "psion-dense-optimizer-corruption-v1",
        dense_artifact.artifact_id.clone(),
        dense_artifact
            .optimizer_state_restart
            .optimizer_state_artifacts[0]
            .manifest_digest
            .clone(),
        PsionCheckpointCorruptionKind::OptimizerStateMismatch,
        "Optimizer-state corruption invalidated the run rather than allowing silent continuation.",
        &dense_artifact,
    )?;
    let corruption_invalidation = record_psion_checkpoint_recovery_event(
        "psion-corruption-invalidation-v1",
        PsionCheckpointRecoveryEventKind::CorruptionDetectedInvalidation,
        dense_artifact.artifact_id.clone(),
        TrainingRecoveryMode::ResumeFromLastStableCheckpoint,
        None,
        "psion-recovery-topology-invalidation-v1",
        1,
        stage_receipt.dataset_identity.clone(),
        stage_receipt.sampling_policy_id.clone(),
        stage_receipt.sampling_policy_version.clone(),
        2048,
        TrainingInstabilityTelemetry::default()
            .with_checkpoint_catchup_latency_ms(910)
            .with_environment_failure_rate_bps(140),
        halt_verdict(910, 140),
        Some(invalidation_corruption),
        PsionCheckpointRecoveryDisposition::Invalidated,
        None,
        "Optimizer-state corruption invalidated the run instead of resuming from a possibly poisoned state.",
        &artifacts,
    )?;

    let bundle = record_psion_checkpoint_recovery_bundle(
        "psion-checkpoint-recovery-bundle-v1",
        artifacts,
        vec![
            forced_restart,
            distributed_restart,
            corruption_rollback,
            corruption_invalidation,
        ],
        dense_artifact.artifact_id.clone(),
        "Psion checkpoint recovery bundle freezes dense restart, sharded distributed restart, corruption rollback, and corruption invalidation over one bounded promoted checkpoint.",
        &stage_receipt,
        &observability_receipt,
    )?;

    fs::write(
        fixtures_dir.join("psion_dense_checkpoint_artifact_v1.json"),
        serde_json::to_vec_pretty(&dense_artifact)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_sharded_checkpoint_artifact_v1.json"),
        serde_json::to_vec_pretty(&sharded_artifact)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_checkpoint_recovery_bundle_v1.json"),
        serde_json::to_vec_pretty(&bundle)?,
    )?;

    Ok(())
}

fn checkpoint_context(
    stage_receipt: &PsionPretrainStageRunReceipt,
    observability_receipt: &PsionPretrainRunObservabilityReceipt,
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
            "Checkpoint artifacts preserve the source checkpoint topology and the realized training hardware topology separately.",
        ),
    }
}

fn dense_optimizer_restart() -> PsionOptimizerStateRestartReceipt {
    PsionOptimizerStateRestartReceipt {
        optimizer_family: String::from("adamw"),
        optimizer_checkpoint_family: String::from("train.psion.decoder.optimizer_state"),
        optimizer_state_step: 2048,
        parameter_group_count: 8,
        optimizer_state_artifacts: vec![checkpoint_stream_ref(
            "stream-psion-pretrain-final-optimizer-v1",
            "manifest-psion-pretrain-final-optimizer-v1",
            "object-psion-pretrain-final-optimizer-v1",
            "train.psion.decoder.optimizer_state",
            "checkpoint://psion/pilot/pretrain/final/optimizer_state",
            2048,
            71_827_456,
        )],
        strict_parameter_group_order_restore: true,
        resume_requires_matching_sampling_cursor: true,
        summary: String::from(
            "Dense optimizer-state restart keeps exact parameter-group order and sampling cursor binding.",
        ),
    }
}

fn sharded_optimizer_restart() -> PsionOptimizerStateRestartReceipt {
    PsionOptimizerStateRestartReceipt {
        optimizer_family: String::from("adamw"),
        optimizer_checkpoint_family: String::from("train.psion.decoder.optimizer_state"),
        optimizer_state_step: 2048,
        parameter_group_count: 8,
        optimizer_state_artifacts: vec![
            checkpoint_stream_ref(
                "stream-psion-pretrain-final-optimizer-shard-0-v1",
                "manifest-psion-pretrain-final-optimizer-shard-0-v1",
                "object-psion-pretrain-final-optimizer-shard-0-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/pilot/pretrain/final/optimizer_state/sharded",
                2048,
                18_000_000,
            ),
            checkpoint_stream_ref(
                "stream-psion-pretrain-final-optimizer-shard-1-v1",
                "manifest-psion-pretrain-final-optimizer-shard-1-v1",
                "object-psion-pretrain-final-optimizer-shard-1-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/pilot/pretrain/final/optimizer_state/sharded",
                2048,
                18_000_000,
            ),
            checkpoint_stream_ref(
                "stream-psion-pretrain-final-optimizer-shard-2-v1",
                "manifest-psion-pretrain-final-optimizer-shard-2-v1",
                "object-psion-pretrain-final-optimizer-shard-2-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/pilot/pretrain/final/optimizer_state/sharded",
                2048,
                18_000_000,
            ),
            checkpoint_stream_ref(
                "stream-psion-pretrain-final-optimizer-shard-3-v1",
                "manifest-psion-pretrain-final-optimizer-shard-3-v1",
                "object-psion-pretrain-final-optimizer-shard-3-v1",
                "train.psion.decoder.optimizer_state",
                "checkpoint://psion/pilot/pretrain/final/optimizer_state/sharded",
                2048,
                18_000_000,
            ),
        ],
        strict_parameter_group_order_restore: true,
        resume_requires_matching_sampling_cursor: true,
        summary: String::from(
            "Sharded optimizer-state restart preserves group order, step identity, and exact sampling cursor binding across four shards.",
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
        &[NodeId::new("node-psion-a")],
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
                signal: psionic_train::TrainingInstabilitySignalKind::CheckpointCatchupLatencyMs,
                max_value: 500.0,
                action: TrainingOperationalAction::Quarantine,
            },
            TrainingInstabilityRule {
                signal: psionic_train::TrainingInstabilitySignalKind::EnvironmentFailureRateBps,
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

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| String::from("workspace root should exist").into())
}
