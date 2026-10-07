use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    PsionActualPretrainingArtifactRef, PsionActualPretrainingDistributedQualification,
    PsionActualPretrainingHardwarePreflightItem, PsionActualPretrainingMemoryQualification,
    PsionActualPretrainingResumeRehearsalSupport, PsionActualPretrainingSystemsBenchmarkBinding,
    PsionActualPretrainingSystemsBundle, PsionActualPretrainingSystemsThroughputBaseline,
    PsionActualPretrainingTopologyStorageBundle, PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH,
    PSION_ACTUAL_PRETRAINING_LANE_ID, PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_DOC_PATH,
    PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_FIXTURE_PATH,
    PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_ID,
    PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let pretrain_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&pretrain_dir)?;

    let lane_spec_path = pretrain_dir.join("psion_actual_pretraining_lane_spec_v1.json");
    let recipe_bundle_path = pretrain_dir.join("psion_actual_pretraining_recipe_bundle_v1.json");
    let topology_bundle_path =
        pretrain_dir.join("psion_actual_pretraining_topology_storage_bundle_v1.json");
    let anchor_run_bundle_path =
        root.join("fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json");
    let observability_receipt_path =
        root.join("fixtures/psion/observability/psion_broader_pretrain_run_observability_receipt_v1.json");
    let checkpoint_recovery_path =
        root.join("fixtures/psion/trusted_cluster/psion_trusted_cluster_checkpoint_recovery_bundle_v1.json");
    let replay_receipt_path =
        root.join("fixtures/psion/trusted_cluster/psion_trusted_cluster_replay_receipt_v1.json");

    let topology: PsionActualPretrainingTopologyStorageBundle = load_json(&topology_bundle_path)?;
    topology.validate()?;
    let anchor_run_bundle: serde_json::Value = load_json(&anchor_run_bundle_path)?;
    let observability_receipt: serde_json::Value = load_json(&observability_receipt_path)?;
    let checkpoint_recovery_bundle: serde_json::Value = load_json(&checkpoint_recovery_path)?;
    let replay_receipt: serde_json::Value = load_json(&replay_receipt_path)?;

    let throughput_baselines = vec![PsionActualPretrainingSystemsThroughputBaseline {
        baseline_id: String::from("psion_actual_pretraining_trusted_cluster_anchor"),
        baseline_kind: String::from("trusted_cluster_anchor"),
        source_profile: json_string(
            &observability_receipt,
            &["run_profile"],
            "observability_receipt.run_profile",
        )?,
        worker_count: json_u64(
            &observability_receipt,
            &["hardware_topology", "observed_worker_count"],
            "observability_receipt.hardware_topology.observed_worker_count",
        )?,
        mean_tokens_per_second: json_u64(
            &observability_receipt,
            &["throughput", "mean_tokens_per_second"],
            "observability_receipt.throughput.mean_tokens_per_second",
        )?,
        peak_tokens_per_second: json_u64(
            &observability_receipt,
            &["throughput", "peak_tokens_per_second"],
            "observability_receipt.throughput.peak_tokens_per_second",
        )?,
        mean_step_latency_ms: json_u64(
            &observability_receipt,
            &["throughput", "mean_step_latency_ms"],
            "observability_receipt.throughput.mean_step_latency_ms",
        )?,
        checkpoint_write_throughput_bytes_per_second: json_u64(
            &observability_receipt,
            &["throughput", "checkpoint_write_throughput_bytes_per_second"],
            "observability_receipt.throughput.checkpoint_write_throughput_bytes_per_second",
        )?,
        source_receipt_id: json_string(
            &observability_receipt,
            &["receipt_id"],
            "observability_receipt.receipt_id",
        )?,
        source_receipt_digest: json_string(
            &observability_receipt,
            &["observability_digest"],
            "observability_receipt.observability_digest",
        )?,
        detail: String::from(
            "The actual lane now binds its systems baseline to the broader-pretraining trusted-cluster observability receipt instead of leaving A2-style profiling as detached prose.",
        ),
    }];

    let selected_devices = json_array(
        &observability_receipt,
        &["hardware_topology", "delivered_execution", "selected_devices"],
        "observability_receipt.hardware_topology.delivered_execution.selected_devices",
    )?;
    let min_free_memory_bytes = selected_devices
        .iter()
        .map(|device| {
            device["free_memory_bytes"]
                .as_u64()
                .ok_or("selected device free_memory_bytes missing")
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .min()
        .ok_or("selected device list must not be empty")?;
    let per_worker_total_memory_bytes = selected_devices[0]["total_memory_bytes"]
        .as_u64()
        .ok_or("selected device total_memory_bytes missing")?;

    let memory_qualification = PsionActualPretrainingMemoryQualification {
        qualification_id: String::from("psion_actual_pretraining_memory_anchor"),
        required_backend: topology.required_backend.clone(),
        worker_count: topology.required_worker_count,
        per_worker_total_memory_bytes,
        min_per_worker_free_memory_bytes: min_free_memory_bytes,
        checkpoint_total_bytes: json_u64(
            &observability_receipt,
            &["checkpoint_artifact", "total_artifact_size_bytes"],
            "observability_receipt.checkpoint_artifact.total_artifact_size_bytes",
        )?,
        optimizer_state_bytes: json_u64(
            &observability_receipt,
            &["checkpoint_artifact", "optimizer_state_size_bytes"],
            "observability_receipt.checkpoint_artifact.optimizer_state_size_bytes",
        )?,
        shard_count: json_u64(
            &observability_receipt,
            &["checkpoint_artifact", "shard_count"],
            "observability_receipt.checkpoint_artifact.shard_count",
        )?,
        activation_posture: String::from("tensor_parallel_anchor_with_checkpointed_activations"),
        detail: String::from(
            "Memory qualification freezes the four-worker H100 headroom and checkpoint-size posture that the actual lane is allowed to claim today.",
        ),
    };

    let distributed_qualification = PsionActualPretrainingDistributedQualification {
        qualification_id: String::from("psion_actual_pretraining_distributed_anchor"),
        topology_storage_bundle_id: topology.bundle_id.clone(),
        supported_topology_label: topology.supported_topology_label.clone(),
        placement_shape: topology.placement_shape.clone(),
        runtime_backend: json_string(
            &observability_receipt,
            &["hardware_topology", "delivered_execution", "runtime_backend"],
            "observability_receipt.hardware_topology.delivered_execution.runtime_backend",
        )?,
        transport: json_string(
            &observability_receipt,
            &[
                "hardware_topology",
                "delivered_execution",
                "cluster_execution",
                "transport",
            ],
            "observability_receipt.hardware_topology.delivered_execution.cluster_execution.transport",
        )?,
        collective_kind: json_string(
            &observability_receipt,
            &[
                "hardware_topology",
                "delivered_execution",
                "cluster_execution",
                "training_collective",
                "kind",
            ],
            "observability_receipt.hardware_topology.delivered_execution.cluster_execution.training_collective.kind",
        )?,
        collective_benchmark_digest: json_string(
            &observability_receipt,
            &[
                "hardware_topology",
                "delivered_execution",
                "cluster_execution",
                "training_collective",
                "benchmark_digest",
            ],
            "observability_receipt.hardware_topology.delivered_execution.cluster_execution.training_collective.benchmark_digest",
        )?,
        replay_receipt_id: json_string(
            &replay_receipt,
            &["receipt_id"],
            "replay_receipt.receipt_id",
        )?,
        replay_receipt_digest: json_string(
            &replay_receipt,
            &["receipt_digest"],
            "replay_receipt.receipt_digest",
        )?,
        exact_replay_observed: json_bool(
            &replay_receipt,
            &["exact_replay_observed"],
            "replay_receipt.exact_replay_observed",
        )?,
        data_feed_contract: String::from(
            "repeat + deterministic_shuffle seed 1337 + trusted-cluster exact replay",
        ),
        distributed_step_receipt_id: json_string(
            &anchor_run_bundle,
            &["distributed_step_receipt", "step_id"],
            "anchor_run_bundle.distributed_step_receipt.step_id",
        )?,
        distributed_step_contract_digest: json_string(
            &anchor_run_bundle,
            &["distributed_step_receipt", "contract_digest"],
            "anchor_run_bundle.distributed_step_receipt.contract_digest",
        )?,
        detail: String::from(
            "Distributed qualification freezes one admitted four-worker CUDA tensor-parallel lane with explicit collective benchmarking, exact replay, and a real distributed-step contract digest.",
        ),
    };

    let hardware_preflight_items = vec![
        PsionActualPretrainingHardwarePreflightItem {
            item_id: String::from("psion_actual_pretraining_backend_family"),
            category: String::from("backend_family"),
            required_evidence_ref: String::from(
                "fixtures/psion/pretrain/psion_actual_pretraining_topology_storage_bundle_v1.json#required_backend",
            ),
            blocking_reason: String::from(
                "the actual lane is admitted only on the frozen CUDA H100 topology",
            ),
            detail: String::from(
                "Operators must prove the selected workers still satisfy the frozen CUDA backend before launch.",
            ),
        },
        PsionActualPretrainingHardwarePreflightItem {
            item_id: String::from("psion_actual_pretraining_worker_inventory"),
            category: String::from("worker_inventory"),
            required_evidence_ref: String::from(
                "fixtures/psion/observability/psion_broader_pretrain_run_observability_receipt_v1.json#hardware_topology",
            ),
            blocking_reason: String::from(
                "actual-lane launch is blocked if the four-node H100 inventory cannot be reproduced",
            ),
            detail: String::from(
                "The systems bundle makes the anchor hardware shape explicit instead of leaving it implied by trusted-cluster history.",
            ),
        },
        PsionActualPretrainingHardwarePreflightItem {
            item_id: String::from("psion_actual_pretraining_storage_credentials"),
            category: String::from("storage_credentials"),
            required_evidence_ref: String::from(
                "fixtures/psion/pretrain/psion_actual_pretraining_topology_storage_bundle_v1.json#credential_sources",
            ),
            blocking_reason: String::from(
                "launch is blocked if the admitted storage env vars and credential file posture are missing",
            ),
            detail: String::from(
                "This keeps A2-style systems prep tied to the actual artifact and checkpoint roots rather than a detached benchmark sandbox.",
            ),
        },
        PsionActualPretrainingHardwarePreflightItem {
            item_id: String::from("psion_actual_pretraining_checkpoint_restore"),
            category: String::from("checkpoint_restore"),
            required_evidence_ref: String::from(
                "fixtures/psion/trusted_cluster/psion_trusted_cluster_checkpoint_recovery_bundle_v1.json",
            ),
            blocking_reason: String::from(
                "resume is blocked if the accepted checkpoint family lacks restart and corruption-drill coverage",
            ),
            detail: String::from(
                "The actual lane now carries one explicit restore and rollback prerequisite drawn from the trusted-cluster anchor bundle.",
            ),
        },
    ];

    let anchor_run_bundle_ref = artifact_ref(&root, &anchor_run_bundle_path)?;
    let observability_ref = artifact_ref(&root, &observability_receipt_path)?;
    let replay_ref = artifact_ref(&root, &replay_receipt_path)?;
    let checkpoint_recovery_ref = artifact_ref(&root, &checkpoint_recovery_path)?;

    let benchmark_bindings = vec![
        PsionActualPretrainingSystemsBenchmarkBinding {
            benchmark_id: String::from("psion_actual_pretraining_throughput_anchor"),
            benchmark_family: String::from("throughput_anchor"),
            source_artifact: observability_ref.clone(),
            source_receipt_id: json_string(
                &observability_receipt,
                &["receipt_id"],
                "observability_receipt.receipt_id",
            )?,
            source_receipt_digest: json_string(
                &observability_receipt,
                &["observability_digest"],
                "observability_receipt.observability_digest",
            )?,
            required_for: String::from("actual-lane throughput baseline"),
            detail: String::from(
                "The actual lane now cites one retained broader-pretraining throughput anchor instead of a detached benchmark note.",
            ),
        },
        PsionActualPretrainingSystemsBenchmarkBinding {
            benchmark_id: String::from("psion_actual_pretraining_collective_sync"),
            benchmark_family: String::from("collective_sync"),
            source_artifact: anchor_run_bundle_ref.clone(),
            source_receipt_id: json_string(
                &anchor_run_bundle,
                &["distributed_step_receipt", "step_id"],
                "anchor_run_bundle.distributed_step_receipt.step_id",
            )?,
            source_receipt_digest: json_string(
                &anchor_run_bundle,
                &["distributed_step_receipt", "receipt_digest"],
                "anchor_run_bundle.distributed_step_receipt.receipt_digest",
            )?,
            required_for: String::from("distributed-runtime qualification"),
            detail: String::from(
                "Collective efficiency work is grounded in the trusted-cluster distributed-step receipt already admitted by the lane anchor.",
            ),
        },
        PsionActualPretrainingSystemsBenchmarkBinding {
            benchmark_id: String::from("psion_actual_pretraining_replay_exactness"),
            benchmark_family: String::from("replay_exactness"),
            source_artifact: replay_ref.clone(),
            source_receipt_id: json_string(
                &replay_receipt,
                &["receipt_id"],
                "replay_receipt.receipt_id",
            )?,
            source_receipt_digest: json_string(
                &replay_receipt,
                &["receipt_digest"],
                "replay_receipt.receipt_digest",
            )?,
            required_for: String::from("dataloader and seed-discipline qualification"),
            detail: String::from(
                "Replay exactness remains a hard systems requirement because the actual lane does not admit distributed data feeds that cannot be replayed exactly.",
            ),
        },
        PsionActualPretrainingSystemsBenchmarkBinding {
            benchmark_id: String::from("psion_actual_pretraining_resume_recovery"),
            benchmark_family: String::from("resume_recovery"),
            source_artifact: checkpoint_recovery_ref.clone(),
            source_receipt_id: json_string(
                &checkpoint_recovery_bundle,
                &["bundle_id"],
                "checkpoint_recovery_bundle.bundle_id",
            )?,
            source_receipt_digest: json_string(
                &checkpoint_recovery_bundle,
                &["bundle_digest"],
                "checkpoint_recovery_bundle.bundle_digest",
            )?,
            required_for: String::from("resume-path rehearsal support"),
            detail: String::from(
                "Resume-path support is bound to the trusted-cluster recovery drills instead of remaining an unfrozen roadmap claim.",
            ),
        },
    ];

    let resume_rehearsal_support = PsionActualPretrainingResumeRehearsalSupport {
        recovery_bundle: checkpoint_recovery_ref,
        recovery_bundle_digest: json_string(
            &checkpoint_recovery_bundle,
            &["bundle_digest"],
            "checkpoint_recovery_bundle.bundle_digest",
        )?,
        required_recovery_event_ids: vec![
            String::from("psion-trusted-cluster-distributed-restart-v1"),
            String::from("psion-trusted-cluster-corruption-rollback-v1"),
            String::from("psion-trusted-cluster-corruption-invalidation-v1"),
        ],
        accepted_pointer_path: String::from("checkpoints/latest_accepted_checkpoint_pointer.json"),
        continuation_handoff_path: String::from(PSION_ACTUAL_PRETRAINING_CONTINUATION_HANDOFF_PATH),
        detail: String::from(
            "The actual lane now binds resume-path support to one retained restart/rollback/invalidation family before later backup automation lands.",
        ),
    };

    let mut systems_bundle = PsionActualPretrainingSystemsBundle {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_SCHEMA_VERSION),
        systems_bundle_id: String::from(PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        lane_spec: artifact_ref(&root, &lane_spec_path)?,
        recipe_bundle: artifact_ref(&root, &recipe_bundle_path)?,
        topology_storage_bundle: artifact_ref(&root, &topology_bundle_path)?,
        anchor_run_bundle: anchor_run_bundle_ref,
        throughput_baselines,
        memory_qualification,
        distributed_qualification,
        hardware_preflight_items,
        benchmark_bindings,
        resume_rehearsal_support,
        support_refs: vec![
            String::from(PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_DOC_PATH),
            String::from("docs/PSION_ACTUAL_PRETRAINING_RUNBOOK.md"),
            String::from("docs/PSION_ACTUAL_PRETRAINING_RECIPE.md"),
            String::from("docs/PSION_TRUSTED_CLUSTER_RUN.md"),
            String::from("docs/PSION_CHECKPOINT_RECOVERY.md"),
            String::from("docs/DISTRIBUTED_DATA_FEED_SEMANTICS.md"),
        ],
        claim_boundary: String::from(
            "This systems bundle ports CS336 A2 concerns into the actual lane by freezing one throughput, memory, distributed-runtime, hardware-preflight, and resume-support authority surface above the admitted trusted-cluster anchor. It does not claim that later hardware admission gates, durable backup automation, or live dashboards are already complete.",
        ),
        summary: String::from(
            "The canonical actual-pretraining systems bundle binds the admitted H100 trusted-cluster throughput, memory headroom, distributed runtime, preflight blockers, and resume drills directly into the actual lane.",
        ),
        bundle_digest: String::new(),
    };
    systems_bundle.bundle_digest = stable_bundle_digest(&systems_bundle)?;
    systems_bundle.validate()?;

    fs::write(
        root.join(PSION_ACTUAL_PRETRAINING_SYSTEMS_BUNDLE_FIXTURE_PATH),
        serde_json::to_string_pretty(&systems_bundle)?,
    )?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}

fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Box<dyn Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn artifact_ref(
    root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let relative = path.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
    Ok(PsionActualPretrainingArtifactRef {
        path: relative,
        sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}

fn stable_bundle_digest(bundle: &PsionActualPretrainingSystemsBundle) -> Result<String, Box<dyn Error>> {
    let mut copy = bundle.clone();
    copy.bundle_digest.clear();
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&copy)?)))
}

fn json_string(
    value: &serde_json::Value,
    path: &[&str],
    field: &str,
) -> Result<String, Box<dyn Error>> {
    navigate(value, path)?
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("{field} missing string").into())
}

fn json_u64(
    value: &serde_json::Value,
    path: &[&str],
    field: &str,
) -> Result<u64, Box<dyn Error>> {
    navigate(value, path)?
        .as_u64()
        .ok_or_else(|| format!("{field} missing u64").into())
}

fn json_bool(
    value: &serde_json::Value,
    path: &[&str],
    field: &str,
) -> Result<bool, Box<dyn Error>> {
    navigate(value, path)?
        .as_bool()
        .ok_or_else(|| format!("{field} missing bool").into())
}

fn json_array<'a>(
    value: &'a serde_json::Value,
    path: &[&str],
    field: &str,
) -> Result<&'a Vec<serde_json::Value>, Box<dyn Error>> {
    navigate(value, path)?
        .as_array()
        .ok_or_else(|| format!("{field} missing array").into())
}

fn navigate<'a>(
    mut value: &'a serde_json::Value,
    path: &[&str],
) -> Result<&'a serde_json::Value, Box<dyn Error>> {
    for segment in path {
        value = value
            .get(*segment)
            .ok_or_else(|| format!("missing json path segment `{segment}`"))?;
    }
    Ok(value)
}
