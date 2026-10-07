use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_environments::EnvironmentPackageKey;
use psionic_eval::EvalArtifact;
use psionic_runtime::RuntimeDispatchPolicy;
use psionic_sandbox::{
    ProviderSandboxEntrypointType, ProviderSandboxEnvironmentVar, ProviderSandboxExecutionClass,
    ProviderSandboxJobRequest, ProviderSandboxResourceRequest,
};
use psionic_train::{
    record_psion_rented_cluster_failure_rehearsal_bundle, record_psion_rented_cluster_runbook,
    record_psion_rented_cluster_stop_condition, ArtifactArchiveClass, ArtifactColdRestoreReceipt,
    ArtifactRetentionProfile, ArtifactStorageSweepReceipt, PolicyRevision,
    PsionCheckpointRecoveryBundle, PsionCheckpointRecoveryEventKind,
    PsionRentedClusterFailureRehearsalBundle, PsionRentedClusterInfraDisposition,
    PsionRentedClusterInfraMode, PsionRentedClusterModeEvaluation, PsionRentedClusterRunAction,
    PsionRentedClusterRunbook, PsionRentedClusterStopConditionKind, RolloutArtifact, RolloutSample,
    RolloutTerminationReason, TrainAdmissionReceipt, TrainArtifactClass,
    TrainArtifactStorageController, TrainBudgetCap, TrainCompletionReceipt, TrainPreemptionMode,
    TrainQueueClass, TrainQueuePolicy, TrainScheduledWorkload, TrainSchedulingAccountingController,
    TrainSchedulingAccountingPolicy, TrainingRecoveryMode,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/rented_cluster");
    fs::create_dir_all(&fixtures_dir)?;

    let recovery_bundle: PsionCheckpointRecoveryBundle =
        serde_json::from_str(&fs::read_to_string(root.join(
            "fixtures/psion/checkpoint_recovery/psion_checkpoint_recovery_bundle_v1.json",
        ))?)?;

    let runbook = build_runbook(&recovery_bundle)?;
    let rehearsal_bundle = build_rehearsal_bundle(&runbook, &recovery_bundle)?;

    fs::write(
        fixtures_dir.join("psion_rented_cluster_runbook_v1.json"),
        serde_json::to_vec_pretty(&runbook)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_rented_cluster_failure_rehearsal_bundle_v1.json"),
        serde_json::to_vec_pretty(&rehearsal_bundle)?,
    )?;

    Ok(())
}

fn build_runbook(
    recovery_bundle: &PsionCheckpointRecoveryBundle,
) -> Result<PsionRentedClusterRunbook, Box<dyn Error>> {
    Ok(record_psion_rented_cluster_runbook(
        "psion-rented-cluster-runbook-v1",
        storage_profiles(),
        scheduling_policy()?,
        1,
        2_500,
        mode_evaluations(),
        "Rented-cluster runbook freezes storage persistence, preemption downgrade, cost stop conditions, and explicit refusal on unsupported infra modes before broader cluster work.",
        recovery_bundle,
    )?)
}

fn build_rehearsal_bundle(
    runbook: &PsionRentedClusterRunbook,
    recovery_bundle: &PsionCheckpointRecoveryBundle,
) -> Result<PsionRentedClusterFailureRehearsalBundle, Box<dyn Error>> {
    let (preemption_admission_receipt, trainer_completion_receipt) =
        scheduling_rehearsal_receipts()?;
    let (checkpoint_storage_artifact_id, sweep_receipts, cold_restore_receipts) =
        storage_rehearsal_receipts(recovery_bundle)?;
    let preemption_stop = record_psion_rented_cluster_stop_condition(
        "psion-rented-cluster-preemption-stop-v1",
        PsionRentedClusterStopConditionKind::PreemptionBudgetExceeded,
        preemption_admission_receipt.workload_id.clone(),
        preemption_admission_receipt.preemptions.len() as u64,
        u64::from(runbook.max_preemption_events_before_downgrade),
        PsionRentedClusterRunAction::DowngradeToResumeOnly,
        "Repeated preemption on rented spot capacity downgrades the run to resume-only posture.",
    )?;
    let cost_stop = record_psion_rented_cluster_stop_condition(
        "psion-rented-cluster-cost-stop-v1",
        PsionRentedClusterStopConditionKind::CostGuardrailExceeded,
        trainer_completion_receipt.workload_id.clone(),
        cost_overrun_bps(&trainer_completion_receipt),
        u64::from(runbook.max_cost_overrun_bps_before_stop),
        PsionRentedClusterRunAction::StopRun,
        "Rented-cluster cost overrun breached the stop threshold and terminated the run instead of hiding the spend drift.",
    )?;
    let recovery_event_receipt_ids = recovery_bundle
        .recovery_events
        .iter()
        .filter(|event| {
            matches!(
                event.event_kind,
                PsionCheckpointRecoveryEventKind::ForcedInterruptionRestart
                    | PsionCheckpointRecoveryEventKind::CorruptionDetectedRollback
            )
        })
        .map(|event| event.receipt_id.clone())
        .collect::<Vec<_>>();
    Ok(record_psion_rented_cluster_failure_rehearsal_bundle(
        "psion-rented-cluster-failure-rehearsal-bundle-v1",
        recovery_event_receipt_ids,
        recovery_bundle.last_stable_artifact_id.clone(),
        checkpoint_storage_artifact_id,
        preemption_admission_receipt,
        trainer_completion_receipt,
        sweep_receipts,
        cold_restore_receipts,
        vec![preemption_stop, cost_stop],
        "Rented-cluster rehearsal bundle binds preemption downgrade, cost stop, checkpoint archive, cold restore, and cited recovery receipts into one bounded failure policy artifact.",
        runbook,
        recovery_bundle,
    )?)
}

fn storage_profiles() -> BTreeMap<TrainArtifactClass, ArtifactRetentionProfile> {
    BTreeMap::from([
        (
            TrainArtifactClass::Checkpoint,
            ArtifactRetentionProfile::new(5_000, 30_000, ArtifactArchiveClass::Restorable, 45_000)
                .with_delete_after_ms(Some(172_800_000)),
        ),
        (
            TrainArtifactClass::EvalArtifact,
            ArtifactRetentionProfile::new(10_000, 60_000, ArtifactArchiveClass::Restorable, 60_000),
        ),
        (
            TrainArtifactClass::LogBundle,
            ArtifactRetentionProfile::new(5_000, 15_000, ArtifactArchiveClass::Ephemeral, 10_000)
                .with_delete_after_ms(Some(3_600_000)),
        ),
    ])
}

fn mode_evaluations() -> Vec<PsionRentedClusterModeEvaluation> {
    vec![
        PsionRentedClusterModeEvaluation {
            infra_mode: PsionRentedClusterInfraMode::SingleRegionOnDemand,
            topology_label: String::from("single_region_homogeneous"),
            disposition: PsionRentedClusterInfraDisposition::Supported,
            required_recovery_mode: None,
            detail: String::from(
                "Single-region on-demand rented clusters stay within the bounded runbook when checkpoint persistence and cold restore are green.",
            ),
        },
        PsionRentedClusterModeEvaluation {
            infra_mode: PsionRentedClusterInfraMode::SingleRegionSpot,
            topology_label: String::from("single_region_spot_preemptible"),
            disposition: PsionRentedClusterInfraDisposition::DowngradedResumeOnly,
            required_recovery_mode: Some(TrainingRecoveryMode::ResumeFromLastStableCheckpoint),
            detail: String::from(
                "Single-region spot capacity is allowed only on explicit resume-from-last-stable-checkpoint posture.",
            ),
        },
        PsionRentedClusterModeEvaluation {
            infra_mode: PsionRentedClusterInfraMode::CrossRegionEphemeral,
            topology_label: String::from("cross_region_mixed_latency"),
            disposition: PsionRentedClusterInfraDisposition::Refused,
            required_recovery_mode: None,
            detail: String::from(
                "Cross-region ephemeral clusters are refused because mixed-latency recovery and storage-loss risk are outside the bounded runbook.",
            ),
        },
        PsionRentedClusterModeEvaluation {
            infra_mode: PsionRentedClusterInfraMode::UntrustedSharedCluster,
            topology_label: String::from("shared_untrusted_fabric"),
            disposition: PsionRentedClusterInfraDisposition::Refused,
            required_recovery_mode: None,
            detail: String::from(
                "Untrusted shared clusters are refused rather than backfilling trusted-cluster claims onto rented infrastructure.",
            ),
        },
    ]
}

fn scheduling_policy() -> Result<TrainSchedulingAccountingPolicy, Box<dyn Error>> {
    let mut policy = TrainSchedulingAccountingPolicy::default();
    policy.global_budget = TrainBudgetCap::new(8, 256 * 1024 * 1024 + 2_000, 400_000);
    policy.queue_policies.insert(
        TrainQueueClass::Realtime,
        TrainQueuePolicy::new(
            9_500,
            TrainPreemptionMode::LowerPriorityOnly,
            RuntimeDispatchPolicy::quantized_decode_default(2),
            TrainQueueClass::Realtime,
        )?,
    );
    policy.queue_policies.insert(
        TrainQueueClass::Background,
        TrainQueuePolicy::new(
            1_000,
            TrainPreemptionMode::Never,
            RuntimeDispatchPolicy {
                max_workers: 1,
                target_batch_work_units: 1,
                max_batch_bytes: 64 * 1024 * 1024,
                park_after_idle_batches: 8,
            },
            TrainQueueClass::Background,
        )?,
    );
    Ok(policy)
}

fn scheduling_rehearsal_receipts(
) -> Result<(TrainAdmissionReceipt, TrainCompletionReceipt), Box<dyn Error>> {
    let environment = EnvironmentPackageKey::new("psion.rented.cluster", "1.0.0");

    let mut preemption_controller = TrainSchedulingAccountingController::new(scheduling_policy()?)?;
    let sandbox = TrainScheduledWorkload::for_sandbox_job(
        &sandbox_job_request("rented-sbx-1", 256),
        &environment,
        TrainQueueClass::Background,
        None,
        1_000,
    );
    preemption_controller.admit(sandbox)?;
    let validator = TrainScheduledWorkload::for_validator_artifact(
        &EvalArtifact::new("rented_health", "eval://psion/rented/health", b"health"),
        &environment,
        TrainQueueClass::Realtime,
        "validator.psion.rented",
        1_100,
    );
    let preemption_receipt = preemption_controller.admit(validator)?;

    let mut trainer_controller = TrainSchedulingAccountingController::new(scheduling_policy()?)?;
    let trainer_workload = TrainScheduledWorkload::for_trainer_batch(
        &trainer_batch("psion-rented-batch", &environment)?,
        &environment,
        TrainQueueClass::Standard,
        2_000,
    );
    let trainer_admission = trainer_controller.admit(trainer_workload)?;
    let trainer_completion = trainer_controller.complete_workload(
        trainer_admission.workload_id.as_str(),
        Some(trainer_admission.estimated_cost_units.saturating_mul(4)),
        2_500,
    )?;
    Ok((preemption_receipt, trainer_completion))
}

fn storage_rehearsal_receipts(
    recovery_bundle: &PsionCheckpointRecoveryBundle,
) -> Result<
    (
        String,
        Vec<ArtifactStorageSweepReceipt>,
        Vec<ArtifactColdRestoreReceipt>,
    ),
    Box<dyn Error>,
> {
    let mut controller = TrainArtifactStorageController::new(storage_profiles())?;
    let recovery_artifact = recovery_bundle
        .checkpoint_artifacts
        .iter()
        .find(|artifact| artifact.artifact_id == recovery_bundle.last_stable_artifact_id)
        .ok_or("recovery bundle missing last stable artifact")?;
    let checkpoint_storage_artifact_id = controller.register_checkpoint(
        recovery_artifact.checkpoint_manifest.shards[0]
            .manifest
            .clone(),
        recovery_artifact.checkpoint_manifest.checkpoint.clone(),
        recovery_artifact.checkpoint_manifest.shards[0]
            .manifest
            .total_bytes,
        0,
    )?;
    let warm = controller.sweep(6_000)?;
    let archived = controller.sweep(40_000)?;
    let requested =
        controller.request_cold_restore(checkpoint_storage_artifact_id.as_str(), 42_000)?;
    let completed =
        controller.complete_cold_restore(checkpoint_storage_artifact_id.as_str(), 60_000)?;
    Ok((
        checkpoint_storage_artifact_id,
        vec![warm, archived],
        vec![requested, completed],
    ))
}

fn trainer_batch(
    batch_id: &str,
    environment: &EnvironmentPackageKey,
) -> Result<psionic_train::TrainerBatch, Box<dyn Error>> {
    Ok(psionic_train::TrainerBatch::assemble(
        batch_id,
        PolicyRevision::new(
            "psion.rented.policy",
            "rev-target",
            "policy-target-digest",
            3_000,
        )
        .with_revision_number(2),
        vec![sample_rollout(
            "rollout-rented",
            "worker-rented",
            environment,
        )],
        4_000,
    )?)
}

fn sample_rollout(
    artifact_id: &str,
    worker_id: &str,
    environment: &EnvironmentPackageKey,
) -> RolloutArtifact {
    RolloutArtifact::new(
        artifact_id,
        worker_id,
        environment.clone(),
        "task-rented",
        PolicyRevision::new("psion.rented.policy", "rev-1", "policy-digest", 1_000)
            .with_revision_number(1),
        vec![RolloutSample::new(1, -0.2, 0.8, 0.6)],
        RolloutTerminationReason::Completed,
        Vec::new(),
        2_000,
    )
    .expect("sample rollout should assemble")
}

fn sandbox_job_request(job_id: &str, memory_limit_mb: u64) -> ProviderSandboxJobRequest {
    ProviderSandboxJobRequest {
        job_id: String::from(job_id),
        provider_id: String::from("provider-rented"),
        compute_product_id: String::from("sandbox.python.exec"),
        execution_class: ProviderSandboxExecutionClass::PythonExec,
        entrypoint_type: ProviderSandboxEntrypointType::InlinePayload,
        entrypoint: String::from("print('psion-rented')"),
        payload: None,
        arguments: Vec::new(),
        workspace_root: PathBuf::from("."),
        expected_outputs: vec![String::from("receipt.json")],
        timeout_request_s: 30,
        network_request: String::from("none"),
        filesystem_request: String::from("workspace_rw"),
        environment: vec![ProviderSandboxEnvironmentVar {
            key: String::from("MODE"),
            value: String::from("rented"),
        }],
        resource_request: ProviderSandboxResourceRequest {
            cpu_limit: Some(2),
            memory_limit_mb: Some(memory_limit_mb),
            disk_limit_mb: Some(512),
        },
        payout_reference: None,
        verification_posture: Some(String::from("local")),
    }
}

fn cost_overrun_bps(receipt: &TrainCompletionReceipt) -> u64 {
    let estimated = receipt.estimated_cost_units.max(1);
    if receipt.actual_cost_units <= estimated {
        return 0;
    }
    receipt
        .actual_cost_units
        .saturating_sub(estimated)
        .saturating_mul(10_000)
        / estimated
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    Ok(manifest_dir
        .ancestors()
        .nth(2)
        .ok_or("workspace root should exist")?
        .to_path_buf())
}
