use std::env;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use nexus_control::{DesktopSessionCreateRequest, DesktopSessionResponse};
use openagents_kernel_core::authority::{
    CreateComputeTrainingRunRequest, RegisterComputeCheckpointFamilyPolicyRequest,
    RegisterComputeEnvironmentPackageRequest, RegisterComputeTrainingPolicyRequest,
    RegisterComputeValidatorPolicyRequest,
};
use openagents_kernel_core::compute::{
    COMPUTE_TRAINING_RUN_DEFINITION_METADATA_ABI_VERSION, ComputeAdapterCheckpointPointer,
    ComputeAdapterContributionDisposition, ComputeAdapterDatasetSlice,
    ComputeAdapterPolicyRevision, ComputeCheckpointBinding, ComputeCheckpointFamilyPolicy,
    ComputeEnvironmentBinding, ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus,
    ComputeProofPosture, ComputeRegistryStatus, ComputeTrainingPolicy, ComputeTrainingReplicaType,
    ComputeTrainingRun, ComputeTrainingRunDefinitionMetadata, ComputeTrainingRunStatus,
    ComputeTrainingWorkClass, ComputeValidatorChallengeLease, ComputeValidatorChallengeResult,
    ComputeValidatorChallengeStatus, ComputeValidatorChallengeVerdict, ComputeValidatorPolicy,
};
use openagents_kernel_core::compute_contracts;
use openagents_kernel_core::pylon_training::{
    PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF, PYLON_TRAINING_GCS_CREDENTIAL_SOURCE,
    PylonTrainingArtifacts, PylonTrainingCheckpointBinding, PylonTrainingCollectiveKind,
    PylonTrainingDatasetAssignment, PylonTrainingElasticBoundary, PylonTrainingManifestRole,
    PylonTrainingRunManifestCommon, PylonTrainingRunManifestV1, PylonTrainingTopology,
    PylonTrainingTopologyBackendFamily, PylonTrainingTrn, pylon_training_assignment_seed,
};
use openagents_kernel_core::receipts::{PolicyContext, ReceiptHints, TraceContext};
use openagents_provider_substrate::{
    PROVIDER_TRAINING_CAPABILITY_ENVELOPE_V2_SCHEMA_VERSION,
    ProviderAdapterTrainingContributorAvailability, ProviderAdapterTrainingExecutionBackend,
    ProviderAdapterTrainingSettlementTrigger, ProviderHostDiskTelemetry, ProviderHostGpuTelemetry,
    ProviderHostMemoryTelemetry, ProviderHostNetworkInterfaceTelemetry,
    ProviderHostTelemetrySnapshot, ProviderTrainingArtifactUploadLatencyClass,
    ProviderTrainingCapabilityEnvelopeV2, ProviderTrainingCapabilityTier,
    ProviderTrainingCapabilityTierProfile, ProviderTrainingLeaseReliabilityClass,
    ProviderTrainingReplayCapability, ProviderTrainingReplicaTypeEligibility,
    ProviderTrainingThroughputBand, ProviderTrainingWorkClassEligibility,
};
use openagents_validator_service as validator_service;
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const NETWORK_ID: &str = "trainnet.cs336.a1.demo";
const TRAINING_RUN_ID: &str = "run.cs336.a1.demo";
const WINDOW_ID: &str = "window.cs336.a1.demo.0001";
const TRAINING_POLICY_REF: &str = "policy://training/cs336/a1-demo/v1";
const VALIDATOR_POLICY_REF: &str = "policy://validator/mvp/v1";
const CHECKPOINT_FAMILY: &str = "decoder";
const CHECKPOINT_REF: &str = "checkpoint://run.cs336.a1.demo/0000";
const DEMO_DISPLAY_NAME: &str = "CS336 A1 Demo";
const DEFAULT_BUCKET_URI: &str = "gs://local-cs336-demo";
const DEFAULT_ENVIRONMENT_VERSION: &str = "2026.04.13";
const DEFAULT_VALIDATOR_NODE_PUBKEY: &str = "validator.cs336.a1.demo.local";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum LocalTrainingNodeRoleClaim {
    Validator,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum LocalTrainingNodeDesiredState {
    Running,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum LocalTrainingNodeProcessState {
    Running,
}

#[derive(Debug, Clone, Serialize)]
struct LocalTrainingNodeAdmissionRequest {
    idempotency_key: String,
    requested_at_ms: i64,
    node_pubkey_hex: String,
    release_id: String,
    node_label: Option<String>,
    role_claims: Vec<LocalTrainingNodeRoleClaim>,
    allowed_networks: Vec<String>,
    build_version: Option<String>,
    build_digest: Option<String>,
    contributor_availability: ProviderAdapterTrainingContributorAvailability,
    capability_tier: ProviderTrainingCapabilityTierProfile,
    capability_envelope_v2: ProviderTrainingCapabilityEnvelopeV2,
    host_telemetry: ProviderHostTelemetrySnapshot,
    active_reputation_labels: Vec<String>,
    settlement_destination: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalTrainingNodeHeartbeatRequest {
    idempotency_key: String,
    recorded_at_ms: i64,
    node_pubkey_hex: String,
    build_digest: String,
    training_run_id: String,
    window_id: String,
    assignment_id: String,
    lease_id: String,
    desired_state: LocalTrainingNodeDesiredState,
    process_state: LocalTrainingNodeProcessState,
    last_heartbeat_at_ms: Option<i64>,
    last_exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalPlanTrainingWindowRequest {
    idempotency_key: String,
    recorded_at_ms: i64,
    training_run_id: String,
    stage_id: String,
    round_index: Option<u64>,
    base_checkpoint_ref: String,
    planned_local_step_count: Option<u64>,
    aggregation_rule: Option<String>,
    aggregation_weight_basis: Option<String>,
    adapter_target_id: String,
    adapter_family: String,
    base_model_ref: String,
    adapter_format: String,
    source_policy_revision: ComputeAdapterPolicyRevision,
    source_checkpoint_pointer: ComputeAdapterCheckpointPointer,
    dataset_slices: Vec<ComputeAdapterDatasetSlice>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalTransitionTrainingWindowRequest {
    idempotency_key: String,
    recorded_at_ms: i64,
    window_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct LocalTrainingWindowContributionInput {
    contribution_id: String,
    assignment_id: String,
    submission_receipt_digest: String,
    artifact_id: String,
    manifest_digest: String,
    object_digest: String,
    artifact_receipt_digest: String,
    provenance_bundle_digest: String,
    security_receipt_digest: String,
    replay_receipt_digest: Option<String>,
    validator_receipt_digest: String,
    validation_reason_codes: Vec<String>,
    validator_disposition: Option<ComputeAdapterContributionDisposition>,
    aggregation_eligibility: Option<String>,
    local_step_count: Option<u64>,
    consumed_token_count: Option<u64>,
    consumed_example_count: Option<u64>,
    aggregation_weight_basis: Option<String>,
    aggregation_weight_value: Option<u64>,
    aggregation_weight_bps: Option<u32>,
    promotion_receipt_digest: Option<String>,
    metadata: Value,
}

#[derive(Debug, Clone, Serialize)]
struct LocalSealTrainingWindowRequest {
    idempotency_key: String,
    recorded_at_ms: i64,
    window_id: String,
    contribution_outcomes: Vec<LocalTrainingWindowContributionInput>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalClaimTrainingValidatorChallengeRequest {
    idempotency_key: String,
    requested_at_ms: i64,
    node_pubkey_hex: String,
    requested_network_id: Option<String>,
    requested_training_run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalFinalizeTrainingValidatorChallengeRequest {
    idempotency_key: String,
    recorded_at_ms: i64,
    node_pubkey_hex: String,
    lease: ComputeValidatorChallengeLease,
    result: ComputeValidatorChallengeResult,
    training_disposition: Option<ComputeAdapterContributionDisposition>,
}

#[derive(Debug, Clone, Serialize)]
struct LocalReconcileTrainingWindowRequest {
    idempotency_key: String,
    recorded_at_ms: i64,
    window_id: String,
    contribution_outcomes: Vec<LocalTrainingWindowContributionInput>,
    held_out_average_score_bps: Option<u32>,
    benchmark_pass_rate_bps: Option<u32>,
    runtime_smoke_passed: Option<bool>,
    aggregated_delta_digest: Option<String>,
    accepted_aggregate_id: Option<String>,
    promoted_checkpoint_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct SeedSummary {
    base_url: String,
    bucket_uri: String,
    environment_ref: String,
    training_policy_ref: String,
    training_run_id: String,
    window_id: String,
    manifest_path: String,
    latest_pointer_path: String,
    checkpoint_manifest_path: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CloseoutSummary {
    base_url: String,
    training_run_id: String,
    window_id: String,
    validator_node_pubkey: String,
    challenge_ids: Vec<String>,
    accepted_outcome_id: Option<String>,
    payout_eligible_closeouts: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct LocalPylonRuntimeState {
    #[serde(default)]
    lease_cache: std::collections::BTreeMap<String, LocalPylonLeaseCacheEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct LocalPylonLeaseCacheEntry {
    manifest_digest: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        bail!(
            "usage: nexus_cs336_a1_demo <seed|closeout> [--base-url <url>] [command-specific args]"
        );
    };

    match command.as_str() {
        "seed" => {
            let base_url = required_flag(&mut args, "--base-url")?;
            let object_store_root = required_flag(&mut args, "--object-store-root")?;
            let pylon_run_root = required_flag(&mut args, "--pylon-run-root")?;
            let bucket_uri = optional_flag(&mut args, "--bucket-uri")
                .unwrap_or_else(|| DEFAULT_BUCKET_URI.to_string());
            ensure_no_extra_args(args)?;
            let summary = seed_demo(
                base_url.as_str(),
                Path::new(object_store_root.as_str()),
                Path::new(pylon_run_root.as_str()),
                bucket_uri.as_str(),
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        "closeout" => {
            let base_url = required_flag(&mut args, "--base-url")?;
            let node_pubkey_hex = required_flag(&mut args, "--node-pubkey")?;
            let assignment_id = required_flag(&mut args, "--assignment-id")?;
            let lease_id = required_flag(&mut args, "--lease-id")?;
            let pylon_run_root = required_flag(&mut args, "--pylon-run-root")?;
            let training_run_id = optional_flag(&mut args, "--training-run-id")
                .unwrap_or_else(|| TRAINING_RUN_ID.to_string());
            let window_id =
                optional_flag(&mut args, "--window-id").unwrap_or_else(|| WINDOW_ID.to_string());
            let validator_node_pubkey = optional_flag(&mut args, "--validator-node-pubkey")
                .unwrap_or_else(|| DEFAULT_VALIDATOR_NODE_PUBKEY.to_string());
            let promoted_checkpoint_ref = optional_flag(&mut args, "--promoted-checkpoint-ref");
            ensure_no_extra_args(args)?;
            let summary = closeout_demo(
                base_url.as_str(),
                Path::new(pylon_run_root.as_str()),
                training_run_id.as_str(),
                window_id.as_str(),
                node_pubkey_hex.as_str(),
                assignment_id.as_str(),
                lease_id.as_str(),
                validator_node_pubkey.as_str(),
                promoted_checkpoint_ref.as_deref(),
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        other => bail!("unknown command '{other}'"),
    }

    Ok(())
}

async fn seed_demo(
    base_url: &str,
    object_store_root: &Path,
    pylon_run_root: &Path,
    bucket_uri: &str,
) -> Result<SeedSummary> {
    let client = Client::new();
    let access_token = create_session(&client, base_url).await?;
    let issued_at_ms = now_ms();
    let recorded_at_ms = issued_at_ms as i64;

    write_demo_artifacts(
        base_url,
        object_store_root,
        pylon_run_root,
        bucket_uri,
        issued_at_ms,
    )?;

    let environment_request =
        compute_contracts::register_compute_environment_package_request_to_proto(
            &build_environment_request(recorded_at_ms),
        )
        .map_err(anyhow::Error::msg)?;
    post_json::<_, Value>(
        &client,
        format!("{base_url}/v1/kernel/compute/environments").as_str(),
        &environment_request,
        Some(access_token.as_str()),
    )
    .await?;

    let checkpoint_request =
        compute_contracts::register_compute_checkpoint_family_policy_request_to_proto(
            &build_checkpoint_policy_request(recorded_at_ms),
        )
        .map_err(anyhow::Error::msg)?;
    post_json::<_, Value>(
        &client,
        format!("{base_url}/v1/kernel/compute/checkpoints/policies").as_str(),
        &checkpoint_request,
        Some(access_token.as_str()),
    )
    .await?;

    let validator_request = compute_contracts::register_compute_validator_policy_request_to_proto(
        &build_validator_policy_request(recorded_at_ms),
    )
    .map_err(anyhow::Error::msg)?;
    post_json::<_, Value>(
        &client,
        format!("{base_url}/v1/kernel/compute/validators/policies").as_str(),
        &validator_request,
        Some(access_token.as_str()),
    )
    .await?;

    let training_policy_request =
        compute_contracts::register_compute_training_policy_request_to_proto(
            &build_training_policy_request(recorded_at_ms),
        )
        .map_err(anyhow::Error::msg)?;
    post_json::<_, Value>(
        &client,
        format!("{base_url}/v1/kernel/compute/training/policies").as_str(),
        &training_policy_request,
        Some(access_token.as_str()),
    )
    .await?;

    let training_run_request = compute_contracts::create_compute_training_run_request_to_proto(
        &build_training_run_request(recorded_at_ms, bucket_uri),
    )
    .map_err(anyhow::Error::msg)?;
    post_json::<_, Value>(
        &client,
        format!("{base_url}/v1/kernel/compute/training/runs").as_str(),
        &training_run_request,
        Some(access_token.as_str()),
    )
    .await?;

    let manifest_path = object_store_root.join(run_manifest_relative_path(bucket_uri)?);
    let latest_pointer_path = object_store_root.join(latest_pointer_relative_path(bucket_uri)?);
    let checkpoint_manifest_path =
        object_store_root.join(checkpoint_manifest_relative_path(bucket_uri, 0)?);
    Ok(SeedSummary {
        base_url: base_url.to_string(),
        bucket_uri: bucket_uri.to_string(),
        environment_ref: PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string(),
        training_policy_ref: TRAINING_POLICY_REF.to_string(),
        training_run_id: TRAINING_RUN_ID.to_string(),
        window_id: WINDOW_ID.to_string(),
        manifest_path: manifest_path.display().to_string(),
        latest_pointer_path: latest_pointer_path.display().to_string(),
        checkpoint_manifest_path: checkpoint_manifest_path.display().to_string(),
    })
}

async fn closeout_demo(
    base_url: &str,
    pylon_run_root: &Path,
    training_run_id: &str,
    window_id: &str,
    _worker_node_pubkey_hex: &str,
    assignment_id: &str,
    lease_id: &str,
    validator_node_pubkey_base: &str,
    promoted_checkpoint_ref: Option<&str>,
) -> Result<CloseoutSummary> {
    let client = Client::new();
    let now_ms = now_ms_i64();
    let closeout_nonce = now_ms;
    let manifest_digest = load_cached_lease_manifest_digest(pylon_run_root, lease_id)?;
    let plan_request = LocalPlanTrainingWindowRequest {
        idempotency_key: format!("idemp.training.window.plan.cs336-a1-demo.{closeout_nonce}"),
        recorded_at_ms: now_ms + 20,
        training_run_id: training_run_id.to_string(),
        stage_id: "sft".to_string(),
        round_index: Some(1),
        base_checkpoint_ref: CHECKPOINT_REF.to_string(),
        planned_local_step_count: Some(4),
        aggregation_rule: Some("weighted_avg".to_string()),
        aggregation_weight_basis: Some("tokens".to_string()),
        adapter_target_id: "adapter.target.cs336.a1.demo".to_string(),
        adapter_family: "openagents.adapter.reference".to_string(),
        base_model_ref: "model://psion/reference-demo".to_string(),
        adapter_format: "openagents.adapter.delta.v1".to_string(),
        source_policy_revision: ComputeAdapterPolicyRevision {
            policy_family: "openagents.adapter.reference".to_string(),
            revision_id: "policy-rev-cs336-a1-demo".to_string(),
            revision_number: Some(1),
            policy_digest: "sha256:policy-cs336-a1-demo".to_string(),
            parent_revision_id: None,
            produced_at_ms: now_ms + 15,
        },
        source_checkpoint_pointer: ComputeAdapterCheckpointPointer {
            scope_kind: "training_run".to_string(),
            scope_id: training_run_id.to_string(),
            checkpoint_family: CHECKPOINT_FAMILY.to_string(),
            checkpoint_ref: CHECKPOINT_REF.to_string(),
            manifest_digest: "sha256:checkpoint-manifest-cs336-a1-demo".to_string(),
            updated_at_ms: now_ms + 15,
            pointer_digest: "sha256:pointer-cs336-a1-demo".to_string(),
        },
        dataset_slices: vec![ComputeAdapterDatasetSlice {
            dataset_id: "dataset://cs336/assignment1/tinystories-demo".to_string(),
            split_name: "train".to_string(),
            slice_id: "slice://cs336/a1/demo/0001".to_string(),
            slice_digest: "sha256:cs336-a1-demo-slice-0001".to_string(),
        }],
    };
    post_json::<_, Value>(
        &client,
        format!("{base_url}/api/training/windows/plan").as_str(),
        &plan_request,
        None,
    )
    .await?;

    let activate_request = LocalTransitionTrainingWindowRequest {
        idempotency_key: format!("idemp.training.window.activate.cs336-a1-demo.{closeout_nonce}"),
        recorded_at_ms: now_ms + 30,
        window_id: window_id.to_string(),
    };
    post_json::<_, Value>(
        &client,
        format!("{base_url}/api/training/windows/{window_id}/activate").as_str(),
        &activate_request,
        None,
    )
    .await?;

    let sealed_contribution = build_contribution_input(
        manifest_digest.as_str(),
        assignment_id,
        "contrib.cs336.a1.demo.0001",
        None,
    );
    let seal_request = LocalSealTrainingWindowRequest {
        idempotency_key: format!("idemp.training.window.seal.cs336-a1-demo.{closeout_nonce}"),
        recorded_at_ms: now_ms + 40,
        window_id: window_id.to_string(),
        contribution_outcomes: vec![sealed_contribution],
    };
    post_json::<_, Value>(
        &client,
        format!("{base_url}/api/training/windows/{window_id}/seal").as_str(),
        &seal_request,
        None,
    )
    .await?;

    let mut challenge_ids = Vec::new();
    for challenge_index in 0..8 {
        let validator_node_pubkey_hex =
            format!("{validator_node_pubkey_base}.{closeout_nonce}.{challenge_index}");
        let validator_build_digest =
            format!("sha256:validator-cs336-a1-demo-{closeout_nonce}-{challenge_index}");
        let validator_admission = build_validator_admission_request(
            validator_node_pubkey_hex.as_str(),
            validator_build_digest.as_str(),
            now_ms + 45 + challenge_index * 10,
            closeout_nonce,
        );
        post_json::<_, Value>(
            &client,
            format!("{base_url}/api/training/nodes/admission").as_str(),
            &validator_admission,
            None,
        )
        .await?;

        let validator_heartbeat = LocalTrainingNodeHeartbeatRequest {
            idempotency_key: format!(
                "idemp.training.heartbeat.{}.{}.{}",
                validator_node_pubkey_hex, validator_build_digest, closeout_nonce
            ),
            recorded_at_ms: now_ms + 46 + challenge_index * 10,
            node_pubkey_hex: validator_node_pubkey_hex.clone(),
            build_digest: validator_build_digest.clone(),
            training_run_id: training_run_id.to_string(),
            window_id: window_id.to_string(),
            assignment_id: assignment_id.to_string(),
            lease_id: lease_id.to_string(),
            desired_state: LocalTrainingNodeDesiredState::Running,
            process_state: LocalTrainingNodeProcessState::Running,
            last_heartbeat_at_ms: Some(now_ms + 46 + challenge_index * 10),
            last_exit_code: None,
        };
        post_json::<_, Value>(
            &client,
            format!("{base_url}/api/training/heartbeats").as_str(),
            &validator_heartbeat,
            None,
        )
        .await?;

        let claim_request = LocalClaimTrainingValidatorChallengeRequest {
            idempotency_key: format!(
                "idemp.training.validator.claim.cs336-a1-demo.{closeout_nonce}.{challenge_index}"
            ),
            requested_at_ms: now_ms + 50 + challenge_index * 10,
            node_pubkey_hex: validator_node_pubkey_hex.clone(),
            requested_network_id: Some(NETWORK_ID.to_string()),
            requested_training_run_id: Some(training_run_id.to_string()),
        };
        let claim: Value = match post_json(
            &client,
            format!("{base_url}/api/training/validator-challenges/claim").as_str(),
            &claim_request,
            None,
        )
        .await
        {
            Ok(value) => value,
            Err(error)
                if error
                    .to_string()
                    .contains("training_validator_challenge_unavailable") =>
            {
                break;
            }
            Err(error) => return Err(error),
        };
        let challenge_id = claim
            .get("challenge_id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("validator challenge claim missing challenge_id"))?
            .to_string();
        let lease: ComputeValidatorChallengeLease = serde_json::from_value(
            claim
                .get("lease")
                .cloned()
                .ok_or_else(|| anyhow!("validator challenge claim missing lease"))?,
        )?;
        let proof_bundle_digest = claim
            .get("challenge")
            .and_then(|value| value.get("request"))
            .and_then(|value| value.get("context"))
            .and_then(|value| value.get("proof_bundle_digest"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("validator challenge claim missing proof_bundle_digest"))?;

        let finalize_request = LocalFinalizeTrainingValidatorChallengeRequest {
            idempotency_key: format!(
                "idemp.training.validator.finalize.cs336-a1-demo.{closeout_nonce}.{challenge_index}"
            ),
            recorded_at_ms: now_ms + 60 + challenge_index * 10,
            node_pubkey_hex: validator_node_pubkey_hex.clone(),
            lease: lease.clone(),
            result: ComputeValidatorChallengeResult {
                challenge_id: challenge_id.clone(),
                proof_bundle_digest: proof_bundle_digest.to_string(),
                protocol_id: validator_service::GPU_FREIVALDS_MERKLE_PROTOCOL_ID.to_string(),
                attempt: lease.attempt,
                status: ComputeValidatorChallengeStatus::Verified,
                verdict: ComputeValidatorChallengeVerdict::Verified,
                reason_code: None,
                detail: "local validator accepted contribution".to_string(),
                created_at_ms: lease.leased_at_ms,
                finalized_at_ms: (now_ms + 60 + challenge_index * 10) as u64,
                challenge_seed_digest: None,
                verified_row_count: None,
                result_digest: "sha256:validator-result-cs336-a1-demo".to_string(),
                challenge_result_ref: format!(
                    "validator_challenge_result:{challenge_id}:{}",
                    lease.attempt
                ),
            },
            training_disposition: Some(ComputeAdapterContributionDisposition::Accepted),
        };
        post_json::<_, Value>(
            &client,
            format!("{base_url}/api/training/validator-challenges/{challenge_id}/finalize")
                .as_str(),
            &finalize_request,
            None,
        )
        .await?;
        challenge_ids.push(challenge_id);
    }
    if challenge_ids.is_empty() {
        bail!("validator challenge claim produced no work for {training_run_id}/{window_id}");
    }

    let reconcile_request = LocalReconcileTrainingWindowRequest {
        idempotency_key: format!("idemp.training.window.reconcile.cs336-a1-demo.{closeout_nonce}"),
        recorded_at_ms: now_ms + 70,
        window_id: window_id.to_string(),
        contribution_outcomes: vec![build_contribution_input(
            manifest_digest.as_str(),
            assignment_id,
            "contrib.cs336.a1.demo.0001",
            Some(ComputeAdapterContributionDisposition::Accepted),
        )],
        held_out_average_score_bps: Some(9_600),
        benchmark_pass_rate_bps: Some(9_800),
        runtime_smoke_passed: Some(true),
        aggregated_delta_digest: Some("sha256:aggregate-cs336-a1-demo".to_string()),
        accepted_aggregate_id: None,
        promoted_checkpoint_ref: promoted_checkpoint_ref.map(ToString::to_string),
    };
    let reconcile: Value = post_json(
        &client,
        format!("{base_url}/api/training/windows/{window_id}/reconcile").as_str(),
        &reconcile_request,
        None,
    )
    .await?;

    let stats: Value = get_json(
        &client,
        format!("{base_url}/api/training/summary").as_str(),
        None,
    )
    .await?;
    let accepted_outcome_id = reconcile
        .get("window")
        .and_then(|value| value.get("accepted_outcome_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let payout_eligible_closeouts = stats
        .get("payout_eligible_closeouts")
        .and_then(Value::as_u64);

    Ok(CloseoutSummary {
        base_url: base_url.to_string(),
        training_run_id: training_run_id.to_string(),
        window_id: window_id.to_string(),
        validator_node_pubkey: validator_node_pubkey_base.to_string(),
        challenge_ids,
        accepted_outcome_id,
        payout_eligible_closeouts,
    })
}

fn build_environment_request(created_at_ms: i64) -> RegisterComputeEnvironmentPackageRequest {
    RegisterComputeEnvironmentPackageRequest {
        idempotency_key: "idemp.cs336.a1.demo.environment".to_string(),
        trace: TraceContext::default(),
        policy: PolicyContext::default(),
        package: ComputeEnvironmentPackage {
            environment_ref: PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string(),
            version: DEFAULT_ENVIRONMENT_VERSION.to_string(),
            family: "training".to_string(),
            display_name: "Psion CS336 A1 Demo".to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms,
            updated_at_ms: created_at_ms + 1_000,
            status: ComputeEnvironmentPackageStatus::Active,
            description: Some("Bounded CS336 assignment 1 demo environment".to_string()),
            package_digest: Some(format!(
                "sha256:{}:{}",
                PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF, DEFAULT_ENVIRONMENT_VERSION
            )),
            dataset_bindings: Vec::new(),
            harness: None,
            rubric_bindings: Vec::new(),
            expected_artifacts: Vec::new(),
            policy_refs: Vec::new(),
            metadata: json!({"lane": "cs336_a1_demo"}),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn build_checkpoint_policy_request(
    created_at_ms: i64,
) -> RegisterComputeCheckpointFamilyPolicyRequest {
    RegisterComputeCheckpointFamilyPolicyRequest {
        idempotency_key: "idemp.cs336.a1.demo.checkpoint-policy".to_string(),
        trace: TraceContext::default(),
        policy: PolicyContext::default(),
        policy_record: ComputeCheckpointFamilyPolicy {
            checkpoint_family: CHECKPOINT_FAMILY.to_string(),
            version: DEFAULT_ENVIRONMENT_VERSION.to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms,
            updated_at_ms: created_at_ms + 100,
            status: ComputeRegistryStatus::Active,
            description: Some("CS336 A1 demo checkpoint family".to_string()),
            source_family: Some("reference".to_string()),
            default_recovery_posture: Some("resume_from_latest".to_string()),
            allowed_environment_refs: vec![
                PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string(),
            ],
            validator_policy_ref: Some(VALIDATOR_POLICY_REF.to_string()),
            retention_policy_ref: Some("policy://retention/cs336/a1-demo".to_string()),
            metadata: json!({"lane": "cs336_a1_demo"}),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn build_validator_policy_request(created_at_ms: i64) -> RegisterComputeValidatorPolicyRequest {
    RegisterComputeValidatorPolicyRequest {
        idempotency_key: "idemp.cs336.a1.demo.validator-policy".to_string(),
        trace: TraceContext::default(),
        policy: PolicyContext::default(),
        policy_record: ComputeValidatorPolicy {
            policy_ref: VALIDATOR_POLICY_REF.to_string(),
            version: DEFAULT_ENVIRONMENT_VERSION.to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms,
            updated_at_ms: created_at_ms + 100,
            status: ComputeRegistryStatus::Active,
            validator_pool_ref: "validator-pool.training".to_string(),
            minimum_validator_count: Some(1),
            challenge_window_ms: Some(5_000),
            required_proof_posture: Some(ComputeProofPosture::ChallengeEligible),
            benchmark_package_refs: Vec::new(),
            metadata: json!({"lane": "cs336_a1_demo"}),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn build_training_policy_request(created_at_ms: i64) -> RegisterComputeTrainingPolicyRequest {
    RegisterComputeTrainingPolicyRequest {
        idempotency_key: "idemp.cs336.a1.demo.training-policy".to_string(),
        trace: TraceContext::default(),
        policy: PolicyContext::default(),
        training_policy: ComputeTrainingPolicy {
            training_policy_ref: TRAINING_POLICY_REF.to_string(),
            version: DEFAULT_ENVIRONMENT_VERSION.to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms,
            updated_at_ms: created_at_ms + 100,
            status: ComputeRegistryStatus::Active,
            environment_refs: vec![PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string()],
            checkpoint_family: CHECKPOINT_FAMILY.to_string(),
            validator_policy_ref: VALIDATOR_POLICY_REF.to_string(),
            benchmark_package_refs: Vec::new(),
            stage_policy_refs: vec!["policy://training/cs336/a1-demo/reference".to_string()],
            metadata: training_policy_metadata(),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn build_training_run_request(
    created_at_ms: i64,
    bucket_uri: &str,
) -> CreateComputeTrainingRunRequest {
    CreateComputeTrainingRunRequest {
        idempotency_key: "idemp.cs336.a1.demo.training-run".to_string(),
        trace: TraceContext::default(),
        policy: PolicyContext::default(),
        training_run: ComputeTrainingRun {
            training_run_id: TRAINING_RUN_ID.to_string(),
            training_policy_ref: TRAINING_POLICY_REF.to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string(),
                environment_version: Some(DEFAULT_ENVIRONMENT_VERSION.to_string()),
                dataset_ref: Some("dataset://cs336/assignment1/tinystories-demo".to_string()),
                rubric_ref: None,
                evaluator_policy_ref: None,
            },
            checkpoint_binding: ComputeCheckpointBinding {
                checkpoint_family: CHECKPOINT_FAMILY.to_string(),
                latest_checkpoint_ref: Some(CHECKPOINT_REF.to_string()),
                recovery_posture: Some("resume_from_latest".to_string()),
            },
            validator_policy_ref: VALIDATOR_POLICY_REF.to_string(),
            work_class: ComputeTrainingWorkClass::SmallModelLocalTraining,
            replica_type: ComputeTrainingReplicaType::SingleNode,
            benchmark_package_refs: Vec::new(),
            product_id: Some("psionic.training.cs336_a1_demo".to_string()),
            capacity_lot_id: Some("lot.training.cs336.a1.demo".to_string()),
            instrument_id: Some("instrument.training.cs336.a1.demo".to_string()),
            delivery_proof_id: Some("delivery.training.cs336.a1.demo".to_string()),
            model_ref: Some("model://psion/reference-demo".to_string()),
            source_ref: Some("artifact://training/cs336/a1-demo/input".to_string()),
            rollout_verification_eval_run_ids: Vec::new(),
            created_at_ms,
            started_at_ms: Some(created_at_ms + 100),
            finalized_at_ms: None,
            expected_step_count: Some(4),
            completed_step_count: Some(0),
            status: ComputeTrainingRunStatus::Running,
            final_checkpoint_ref: None,
            promotion_checkpoint_ref: None,
            summary: None,
            metadata: json!({
                "display_name": DEMO_DISPLAY_NAME,
                "pylon_training_scheduler": {
                    "network_id": NETWORK_ID,
                    "artifact_bucket_uri": bucket_uri,
                    "worker_count": 1,
                    "validator_count": 1,
                    "recovery_source_count": 0,
                    "initial_window_id": WINDOW_ID,
                    "checkpoint_ref": CHECKPOINT_REF
                }
            }),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn training_policy_metadata() -> Value {
    json!({
        "run_definition": serde_json::to_value(ComputeTrainingRunDefinitionMetadata {
            abi_version: COMPUTE_TRAINING_RUN_DEFINITION_METADATA_ABI_VERSION.to_string(),
            run_definition_ref: "rundef.cs336.assignment1.demo.v1".to_string(),
            training_family: "psion_reference_demo".to_string(),
            objective: "stanford_cs336_assignment1_demo".to_string(),
            sync_profile: "single_host_reference".to_string(),
            dataset_identity: "dataset://cs336/assignment1/tinystories-demo".to_string(),
            dataset_slice_family: Some("dataset_slice_family.cs336_assignment1_demo".to_string()),
            page_proof_family: Some("cs336.assignment1.demo_page_proof_family".to_string()),
            benchmark_package_set_ref: None,
            version_semantics: "training_policy_version".to_string(),
            window_ref_family: Some("window.family.cs336_assignment1_demo".to_string()),
            manifest_ref_family: Some("manifest.family.psionic_train".to_string()),
            trn_ref_family: Some("trn.family.cs336_assignment1_demo".to_string()),
            closeout_ref_family: Some("closeout.family.accepted_training".to_string()),
        }).expect("training policy metadata"),
    })
}

fn build_validator_admission_request(
    node_pubkey_hex: &str,
    build_digest: &str,
    requested_at_ms: i64,
    closeout_nonce: i64,
) -> LocalTrainingNodeAdmissionRequest {
    let capability_tier = ProviderTrainingCapabilityTierProfile {
        tier: ProviderTrainingCapabilityTier::Tier1Validation,
        backend_families: vec!["cpu".to_string()],
        accelerator_inventory: Vec::new(),
        memory_floor_gb: None,
        available_memory_gb: Some(128),
        throughput_band: ProviderTrainingThroughputBand::Unknown,
        lease_reliability: ProviderTrainingLeaseReliabilityClass::Steady,
        replay_capability: ProviderTrainingReplayCapability::FullWindow,
        artifact_upload_latency_class: ProviderTrainingArtifactUploadLatencyClass::Unknown,
    };
    let contributor_availability = ProviderAdapterTrainingContributorAvailability {
        contributor_supported: true,
        coordinator_match_supported: true,
        authority_receipt_supported: true,
        execution_backends: vec![ProviderAdapterTrainingExecutionBackend::OpenAdapterBackend],
        adapter_families: vec!["openagents.adapter.reference".to_string()],
        adapter_formats: vec!["openagents.adapter.delta.v1".to_string()],
        validator_policy_refs: vec![VALIDATOR_POLICY_REF.to_string()],
        checkpoint_families: vec![CHECKPOINT_FAMILY.to_string()],
        environment_refs: vec![PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string()],
        minimum_memory_gb: None,
        available_memory_gb: Some(128),
        settlement_trigger: Some(ProviderAdapterTrainingSettlementTrigger::AcceptedSealedWindow),
    };
    let capability_envelope_v2 = ProviderTrainingCapabilityEnvelopeV2 {
        schema_version: PROVIDER_TRAINING_CAPABILITY_ENVELOPE_V2_SCHEMA_VERSION.to_string(),
        tier_profile: capability_tier.clone(),
        runtime_surface_detected: true,
        contributor_supported: true,
        benchmark_lane_available: true,
        eligible_work_classes: vec![ProviderTrainingWorkClassEligibility {
            work_class: ComputeTrainingWorkClass::ValidationReplay,
            minimum_tier: ProviderTrainingCapabilityTier::Tier1Validation,
            replica_types: vec![ComputeTrainingReplicaType::SingleNode],
            required_backend_families: vec!["cpu".to_string()],
            minimum_memory_gb: None,
            required_throughput_band: ProviderTrainingThroughputBand::Unknown,
            required_replay_capability: ProviderTrainingReplayCapability::ShortWindow,
            benchmark_lane_required: true,
        }],
        eligible_replica_types: vec![ProviderTrainingReplicaTypeEligibility {
            replica_type: ComputeTrainingReplicaType::SingleNode,
            minimum_tier: ProviderTrainingCapabilityTier::Tier1Validation,
            required_backend_families: vec!["cpu".to_string()],
            minimum_memory_gb: None,
        }],
    };
    LocalTrainingNodeAdmissionRequest {
        idempotency_key: format!(
            "idemp.training.admission.{node_pubkey_hex}.{build_digest}.{closeout_nonce}"
        ),
        requested_at_ms,
        node_pubkey_hex: node_pubkey_hex.to_string(),
        release_id: "openagents.pylon@0.1.1-local-demo".to_string(),
        node_label: Some("validator-cs336-a1-demo".to_string()),
        role_claims: vec![LocalTrainingNodeRoleClaim::Validator],
        allowed_networks: vec![NETWORK_ID.to_string()],
        build_version: Some("0.1.1".to_string()),
        build_digest: Some(build_digest.to_string()),
        contributor_availability,
        capability_tier,
        capability_envelope_v2,
        host_telemetry: ProviderHostTelemetrySnapshot {
            captured_at_unix_ms: now_ms(),
            host_name: Some("local-validator".to_string()),
            os_version: Some("macOS".to_string()),
            kernel_version: None,
            cpu_arch: Some("arm64".to_string()),
            physical_cpu_count: Some(8),
            logical_cpu_count: 16,
            cpu_brand: Some("Apple Silicon".to_string()),
            cpu_frequency_mhz: None,
            cpu_usage_percent: Some(12.0),
            load_average: None,
            memory: Some(ProviderHostMemoryTelemetry {
                used_bytes: 32 * 1024 * 1024 * 1024,
                available_bytes: 96 * 1024 * 1024 * 1024,
                total_bytes: 128 * 1024 * 1024 * 1024,
            }),
            swap: None,
            uptime_seconds: Some(3_600),
            gpus: vec![ProviderHostGpuTelemetry {
                model: "Apple M5 Max".to_string(),
                vendor: Some("Apple".to_string()),
                memory_total_bytes: Some(128 * 1024 * 1024 * 1024),
                ..ProviderHostGpuTelemetry::default()
            }],
            disks: vec![ProviderHostDiskTelemetry {
                mount_point: "/".to_string(),
                available_space_bytes: 256 * 1024 * 1024 * 1024,
                total_space_bytes: 512 * 1024 * 1024 * 1024,
                pylon_home_disk: true,
                ..ProviderHostDiskTelemetry::default()
            }],
            network_interfaces: vec![ProviderHostNetworkInterfaceTelemetry {
                name: "lo0".to_string(),
                ..ProviderHostNetworkInterfaceTelemetry::default()
            }],
            thermal_components: Vec::new(),
            power: None,
        },
        active_reputation_labels: Vec::new(),
        settlement_destination: Some("spark:validator-local".to_string()),
    }
}

fn build_contribution_input(
    manifest_digest: &str,
    assignment_id: &str,
    contribution_id: &str,
    validator_disposition: Option<ComputeAdapterContributionDisposition>,
) -> LocalTrainingWindowContributionInput {
    LocalTrainingWindowContributionInput {
        contribution_id: contribution_id.to_string(),
        assignment_id: assignment_id.to_string(),
        submission_receipt_digest: format!("sha256:submission:{contribution_id}"),
        artifact_id: format!("artifact.{contribution_id}"),
        manifest_digest: manifest_digest.to_string(),
        object_digest: format!("sha256:object:{contribution_id}"),
        artifact_receipt_digest: format!("sha256:artifact-receipt:{contribution_id}"),
        provenance_bundle_digest: format!("sha256:provenance:{contribution_id}"),
        security_receipt_digest: format!("sha256:security:{contribution_id}"),
        replay_receipt_digest: Some(format!("sha256:replay:{contribution_id}")),
        validator_receipt_digest: format!("sha256:validator:{contribution_id}"),
        validation_reason_codes: Vec::new(),
        validator_disposition,
        aggregation_eligibility: None,
        local_step_count: Some(4),
        consumed_token_count: Some(4_096),
        consumed_example_count: Some(64),
        aggregation_weight_basis: Some("tokens".to_string()),
        aggregation_weight_value: Some(4_096),
        aggregation_weight_bps: Some(10_000),
        promotion_receipt_digest: None,
        metadata: json!({"contribution_id": contribution_id}),
    }
}

fn load_cached_lease_manifest_digest(pylon_run_root: &Path, lease_id: &str) -> Result<String> {
    let runtime_state_path = pylon_run_root.join("state").join("runtime-state.json");
    let payload = fs::read(runtime_state_path.as_path()).with_context(|| {
        format!(
            "failed to read local pylon training runtime state {}",
            runtime_state_path.display()
        )
    })?;
    let state: LocalPylonRuntimeState = serde_json::from_slice(payload.as_slice())
        .context("failed to parse pylon runtime state")?;
    state
        .lease_cache
        .get(lease_id)
        .and_then(|lease| lease.manifest_digest.clone())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow!(
                "lease `{lease_id}` is missing a cached manifest digest in {}",
                runtime_state_path.display()
            )
        })
}

fn write_demo_artifacts(
    base_url: &str,
    object_store_root: &Path,
    pylon_run_root: &Path,
    bucket_uri: &str,
    issued_at_ms: u64,
) -> Result<()> {
    let dataset_slice = ComputeAdapterDatasetSlice {
        dataset_id: "dataset://cs336/assignment1/tinystories-demo".to_string(),
        split_name: "train".to_string(),
        slice_id: "slice://cs336/a1/demo/0001".to_string(),
        slice_digest: "sha256:cs336-a1-demo-slice-0001".to_string(),
    };
    let assignment_seed = pylon_training_assignment_seed(
        TRAINING_RUN_ID,
        WINDOW_ID,
        "members.rev1",
        "assign.run.cs336.a1.demo.window.cs336.a1.demo.0001.worker.0.attempt1",
        "worker.cs336.a1.demo",
        &dataset_slice,
    )
    .map_err(anyhow::Error::msg)?;

    let topology = PylonTrainingTopology {
        backend_family: PylonTrainingTopologyBackendFamily::Mlx,
        world_size: 1,
        rank: 0,
        local_device_ids: vec![0],
        collective_kind: PylonTrainingCollectiveKind::DataParallel,
        elastic_boundary: PylonTrainingElasticBoundary::Window,
    };
    let common = PylonTrainingRunManifestCommon {
        manifest_id: "manifest.cs336.a1.demo.worker".to_string(),
        issued_at_ms,
        expires_at_ms: issued_at_ms + 600_000,
        network_id: NETWORK_ID.to_string(),
        run_id: TRAINING_RUN_ID.to_string(),
        window_id: WINDOW_ID.to_string(),
        assignment_id: "assign.run.cs336.a1.demo.window.cs336.a1.demo.0001.worker.0.attempt1"
            .to_string(),
        lease_id: "lease.run.cs336.a1.demo.window.cs336.a1.demo.0001.worker.0.attempt1.rev1"
            .to_string(),
        lease_sequence: 1,
        membership_revision: "members.rev1".to_string(),
        node_pubkey: "11".repeat(32),
        coordinator_pubkey: "22".repeat(32),
        authority_base_url: base_url.to_string(),
        training_policy_ref: TRAINING_POLICY_REF.to_string(),
        validator_policy_ref: VALIDATOR_POLICY_REF.to_string(),
        environment_ref: PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string(),
        environment_version: DEFAULT_ENVIRONMENT_VERSION.to_string(),
    };
    let artifacts = PylonTrainingArtifacts {
        bucket_uri: bucket_uri.to_string(),
        run_prefix: format!("networks/{NETWORK_ID}/runs/{TRAINING_RUN_ID}"),
        window_prefix: format!("networks/{NETWORK_ID}/runs/{TRAINING_RUN_ID}/windows/{WINDOW_ID}"),
        local_run_root: pylon_run_root
            .join("runs")
            .join(TRAINING_RUN_ID)
            .display()
            .to_string(),
        credential_source: PYLON_TRAINING_GCS_CREDENTIAL_SOURCE.to_string(),
    };
    let checkpoint = PylonTrainingCheckpointBinding {
        checkpoint_family: CHECKPOINT_FAMILY.to_string(),
        checkpoint_ref: CHECKPOINT_REF.to_string(),
        manifest_digest: "sha256:checkpoint-manifest-cs336-a1-demo".to_string(),
        latest_pointer_ref: format!("{bucket_uri}/{}", latest_pointer_relative_path(bucket_uri)?),
    };
    let trn = PylonTrainingTrn {
        network_coordinate: format!("39500:{}:{NETWORK_ID}", "22".repeat(32)),
        window_coordinate: format!("39510:{}:{WINDOW_ID}", "22".repeat(32)),
        relay_urls: vec!["wss://relay.damus.io".to_string()],
    };
    let manifest = PylonTrainingRunManifestV1::builder(
        PylonTrainingManifestRole::Worker,
        common,
        topology,
        checkpoint,
        artifacts,
        trn,
    )
    .dataset(PylonTrainingDatasetAssignment {
        dataset_id: dataset_slice.dataset_id.clone(),
        slice_id: dataset_slice.slice_id.clone(),
        slice_digest: dataset_slice.slice_digest.clone(),
        assignment_seed,
    })
    .build()
    .map_err(anyhow::Error::msg)?;

    let run_manifest_path = object_store_root.join(run_manifest_relative_path(bucket_uri)?);
    let latest_pointer_path = object_store_root.join(latest_pointer_relative_path(bucket_uri)?);
    let checkpoint_manifest_path =
        object_store_root.join(checkpoint_manifest_relative_path(bucket_uri, 0)?);
    write_json_bytes(
        run_manifest_path.as_path(),
        manifest
            .canonical_json_bytes()
            .map_err(anyhow::Error::msg)?,
    )?;
    write_json_value(
        latest_pointer_path.as_path(),
        &json!({
            "schema_version": "openagents.pylon_training.latest_pointer.v1",
            "checkpoint_ref": CHECKPOINT_REF,
            "checkpoint_label": "checkpoint-0000",
            "optimizer_step": 0
        }),
    )?;
    write_json_value(
        checkpoint_manifest_path.as_path(),
        &json!({
            "schema_version": "openagents.pylon_training.checkpoint_manifest.v1",
            "checkpoint_ref": CHECKPOINT_REF,
            "checkpoint_label": "checkpoint-0000",
            "optimizer_step": 0
        }),
    )?;
    Ok(())
}

fn run_manifest_relative_path(bucket_uri: &str) -> Result<String> {
    Ok(format!(
        "{}/networks/{NETWORK_ID}/runs/{TRAINING_RUN_ID}/manifests/run_manifest.json",
        bucket_name(bucket_uri)?
    ))
}

fn latest_pointer_relative_path(bucket_uri: &str) -> Result<String> {
    Ok(format!(
        "{}/networks/{NETWORK_ID}/runs/{TRAINING_RUN_ID}/checkpoints/latest_pointer.json",
        bucket_name(bucket_uri)?
    ))
}

fn checkpoint_manifest_relative_path(bucket_uri: &str, optimizer_step: u64) -> Result<String> {
    Ok(format!(
        "{}/networks/{NETWORK_ID}/runs/{TRAINING_RUN_ID}/checkpoints/step-{optimizer_step}/checkpoint_manifest.json",
        bucket_name(bucket_uri)?
    ))
}

fn bucket_name(bucket_uri: &str) -> Result<&str> {
    bucket_uri
        .trim()
        .strip_prefix("gs://")
        .map(|value| value.trim_matches('/'))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("invalid bucket uri {bucket_uri}"))
}

fn write_json_value(path: &Path, value: &Value) -> Result<()> {
    write_json_bytes(path, serde_json::to_vec_pretty(value)?)
}

fn write_json_bytes(path: &Path, bytes: Vec<u8>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create parent {}", parent.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("failed to write {}", path.display()))
}

async fn create_session(client: &Client, base_url: &str) -> Result<String> {
    let response: DesktopSessionResponse = post_json(
        client,
        format!("{base_url}/api/session/desktop").as_str(),
        &DesktopSessionCreateRequest {
            desktop_client_id: "nexus-cs336-a1-demo".to_string(),
            device_name: Some("Local CS336 A1 Demo".to_string()),
            bound_nostr_pubkey: None,
            client_version: Some("local-demo".to_string()),
        },
        None,
    )
    .await?;
    Ok(response.access_token)
}

async fn post_json<T, R>(client: &Client, url: &str, request: &T, bearer: Option<&str>) -> Result<R>
where
    T: Serialize + ?Sized,
    R: DeserializeOwned,
{
    let mut http = client.post(url).header(CONTENT_TYPE, "application/json");
    if let Some(token) = bearer {
        http = http.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = http
        .body(serde_json::to_vec(request)?)
        .send()
        .await
        .with_context(|| format!("failed to POST {url}"))?;
    decode_response(response).await
}

async fn get_json<R>(client: &Client, url: &str, bearer: Option<&str>) -> Result<R>
where
    R: DeserializeOwned,
{
    let mut http = client.get(url);
    if let Some(token) = bearer {
        http = http.header(AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = http
        .send()
        .await
        .with_context(|| format!("failed to GET {url}"))?;
    decode_response(response).await
}

async fn decode_response<R>(response: reqwest::Response) -> Result<R>
where
    R: DeserializeOwned,
{
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("request failed with {}: {}", status, body);
    }
    serde_json::from_str(body.as_str())
        .with_context(|| format!("failed to decode response body: {body}"))
}

fn required_flag(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String> {
    match args.next() {
        Some(name) if name == flag => args
            .next()
            .ok_or_else(|| anyhow!("missing value for {flag}")),
        Some(other) => bail!("expected {flag}, got {other}"),
        None => bail!("missing required flag {flag}"),
    }
}

fn optional_flag(args: &mut impl Iterator<Item = String>, flag: &str) -> Option<String> {
    let next = args.next()?;
    if next != flag {
        return None;
    }
    args.next()
}

fn ensure_no_extra_args(mut args: impl Iterator<Item = String>) -> Result<()> {
    if let Some(extra) = args.next() {
        bail!("unexpected extra argument {extra}");
    }
    Ok(())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis() as u64
}

fn now_ms_i64() -> i64 {
    now_ms() as i64
}
