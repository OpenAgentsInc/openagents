use std::{error::Error, fs, path::PathBuf};

use psionic_data::{DatasetSplitKind, PsionTokenizedCorpusManifest};
use psionic_models::PsionCompactDecoderDescriptor;
use psionic_runtime::{
    ClusterCommunicationClass, ClusterExecutionContext, ClusterExecutionDisposition,
    ClusterSelectedNode, ClusterTransportClass, DeliveredExecutionContext,
    DeviceInventoryQualifiers, DeviceMemoryClass, DevicePerformanceClass, ExecutionTopologyPlan,
    TrainingCheckpointAvailability, TrainingCheckpointReference, TrainingCollectiveContext,
    TrainingCollectiveKind, TrainingCollectiveQuantization, TrainingDeviceMeshContext,
    TrainingElasticMembershipContext, TrainingRecoveryContext, TrainingRecoveryPosture,
};
use psionic_train::{
    record_psion_pretrain_run_observability, run_psion_pretrain_stage,
    summarize_psion_pretrain_observability_runs, PsionPretrainCheckpointArtifactReceipt,
    PsionPretrainCheckpointLineageReceipt, PsionPretrainHardwareTopologyReceipt,
    PsionPretrainLossNormalization, PsionPretrainObjectiveConfig, PsionPretrainObjectiveKind,
    PsionPretrainReplayReceipt, PsionPretrainRunCostBasis, PsionPretrainRunCostReceipt,
    PsionPretrainRunScaleProfile, PsionPretrainRunThroughputReceipt,
    PsionPretrainSourceFamilyReportRow, PsionPretrainStageConfig, PsionPretrainStageRunReceipt,
    PsionSamplingPolicyManifest, TrainingInstabilityPolicy, TrainingInstabilityRule,
    TrainingInstabilitySignalKind, TrainingInstabilityTelemetry, TrainingOperationalAction,
    TrainingStabilityController,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/observability");
    fs::create_dir_all(&fixtures_dir)?;

    let pilot_stage_receipt: PsionPretrainStageRunReceipt =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/pretrain/psion_pretrain_stage_receipt_v1.json"),
        )?)?;
    let broader_stage_receipt = broader_stage_receipt(&root)?;

    let pilot_receipt = record_psion_pretrain_run_observability(
        "psion-pilot-pretrain-observability-v1",
        PsionPretrainRunScaleProfile::Pilot,
        PsionPretrainRunCostReceipt {
            cost_basis: PsionPretrainRunCostBasis::EstimatedUsd,
            currency_code: String::from("USD"),
            compute_cost_microusd: 18_000_000,
            storage_cost_microusd: 750_000,
            network_cost_microusd: 200_000,
            total_cost_microusd: 18_950_000,
            detail: String::from(
                "Pilot run cost is estimated from one bounded single-device accelerator reservation plus local checkpoint storage.",
            ),
        },
        PsionPretrainRunThroughputReceipt {
            train_tokens_processed: 4_194_304,
            validation_tokens_processed: 262_144,
            held_out_tokens_scored: 65_536,
            optimizer_steps_completed: 2048,
            wall_clock_ms: 86_400,
            mean_tokens_per_second: 52_105,
            peak_tokens_per_second: 61_440,
            mean_sequences_per_second_milli: 12_750,
            mean_step_latency_ms: 42,
            checkpoint_write_throughput_bytes_per_second: 402_653_184,
        },
        PsionPretrainCheckpointArtifactReceipt {
            promoted_checkpoint_label: pilot_stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint_label
                .clone(),
            checkpoint_family: pilot_stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .checkpoint_family
                .clone(),
            checkpoint_object_digest: pilot_stage_receipt
                .checkpoint_lineage
                .promoted_checkpoint
                .object_digest
                .clone(),
            checkpoint_size_bytes: 143_654_912,
            optimizer_state_size_bytes: 71_827_456,
            ancillary_artifact_size_bytes: 3_145_728,
            total_artifact_size_bytes: 218_628_096,
            shard_count: 1,
            detail: String::from(
                "Pilot artifact surface stays compact enough for single-host checkpoint rehearsal and replay checks.",
            ),
        },
        PsionPretrainHardwareTopologyReceipt::new(
            1,
            {
                let device = device(
                    "cuda:h100-pilot-0",
                    "0000:81:00.0",
                    80 * 1024 * 1024 * 1024,
                    62 * 1024 * 1024 * 1024,
                );
                DeliveredExecutionContext::new(
                    "cuda",
                    Some(ExecutionTopologyPlan::single_device("cuda", device.clone())),
                    vec![device],
                )
            },
            "Pilot run stayed on one discrete-accelerator lane with an explicit single-device topology contract.",
        )?,
        TrainingInstabilityTelemetry::default().with_checkpoint_catchup_latency_ms(180),
        None,
        "Pilot pretraining observability receipt records the minimum cost, throughput, checkpoint, and hardware facts for the first bounded run.",
        &pilot_stage_receipt,
    )?;

    let broader_receipt = broader_run_observability(&broader_stage_receipt)?;
    let stage_summary = summarize_psion_pretrain_observability_runs(
        "psion-pretrain-stage-observability-v1",
        &[pilot_receipt.clone(), broader_receipt.clone()],
        "Pilot and broader-pretraining observability receipts now expose the minimum budgeting, checkpoint, topology, and instability surface required for Psion scale-up decisions.",
    )?;

    fs::write(
        fixtures_dir.join("psion_pilot_pretrain_run_observability_receipt_v1.json"),
        serde_json::to_string_pretty(&pilot_receipt)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_broader_pretrain_run_observability_receipt_v1.json"),
        serde_json::to_string_pretty(&broader_receipt)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_pretrain_stage_observability_summary_v1.json"),
        serde_json::to_string_pretty(&stage_summary)?,
    )?;
    Ok(())
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
    let membership = TrainingElasticMembershipContext::new(
        7,
        "cluster-state-digest-psion-broad-v1",
        "topology-digest-psion-broad-v1",
        vec![
            String::from("worker-a"),
            String::from("worker-b"),
            String::from("worker-c"),
            String::from("worker-d"),
        ],
    );
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
            vec![
                String::from("worker-a"),
                String::from("worker-b"),
                String::from("worker-c"),
                String::from("worker-d"),
            ],
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
        .ok_or_else(|| "failed to resolve workspace root".into())
}
