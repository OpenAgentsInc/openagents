use anyhow::{Context, Result, anyhow, bail};
use nexus_control::{DesktopSessionCreateRequest, DesktopSessionResponse};
use openagents_kernel_core::authority::{
    HttpKernelAuthorityClient, KernelAuthority, RegisterComputeCheckpointFamilyPolicyRequest,
    RegisterComputeEnvironmentPackageRequest, RegisterComputeTrainingPolicyRequest,
    RegisterComputeValidatorPolicyRequest,
};
use openagents_kernel_core::compute::{
    COMPUTE_TRAINING_RUN_DEFINITION_METADATA_ABI_VERSION, ComputeCheckpointBinding,
    ComputeCheckpointFamilyPolicy, ComputeEnvironmentArtifactExpectation,
    ComputeEnvironmentBinding, ComputeEnvironmentDatasetBinding, ComputeEnvironmentHarness,
    ComputeEnvironmentPackage, ComputeEnvironmentPackageStatus, ComputeEnvironmentRubricBinding,
    ComputeProofPosture, ComputeRegistryStatus, ComputeTrainingPolicy, ComputeTrainingReplicaType,
    ComputeTrainingRun, ComputeTrainingRunDefinitionMetadata, ComputeTrainingRunStatus,
    ComputeTrainingWorkClass, ComputeValidatorPolicy,
};
use openagents_kernel_core::pylon_training::PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF;
use openagents_kernel_core::receipts::{PolicyContext, ReceiptHints, TraceContext};
use reqwest::Client;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_BASE_URL: &str = "http://127.0.0.1:8080";
const DEFAULT_DESKTOP_CLIENT_ID: &str = "episode223-cs336-a1-seeder";
const DEFAULT_DEVICE_NAME: &str = "Episode 223 Seeder";
const DEFAULT_CLIENT_VERSION: &str = "episode223-cs336-a1-seeder/v1";
const DEFAULT_ENVIRONMENT_VERSION: &str = "2026.04.13";
const DEFAULT_POLICY_VERSION: &str = "2026.04.13";
const PROD_ARTIFACT_BUCKET_URI: &str = "gs://openagentsgemini-openagents-training-prod";
const LOCAL_ARTIFACT_BUCKET_URI: &str = "gs://bucket";
const TRAINING_RUN_ID: &str = "run.cs336.a1.demo";
const NETWORK_ID: &str = "trainnet.cs336.a1.demo";
const WINDOW_ID: &str = "window.cs336.a1.demo.0001";
const TRAINING_POLICY_REF: &str = "policy://training/cs336/a1-demo/v1";
const VALIDATOR_POLICY_REF: &str = "policy://validator/mvp/v1";
const CHECKPOINT_FAMILY: &str = "decoder";
const BASE_CHECKPOINT_REF: &str = "checkpoint://decoder/base";

fn main() -> Result<()> {
    ensure_rustls_crypto_provider()?;
    let args = BootstrapArgs::parse(std::env::args().skip(1))?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;
    runtime.block_on(async_main(args))
}

async fn async_main(args: BootstrapArgs) -> Result<()> {
    let session = mint_desktop_session(&args).await?;
    let client =
        HttpKernelAuthorityClient::new(args.base_url.clone(), Some(session.access_token.clone()))?;

    ensure_environment(&client, &args).await?;
    ensure_checkpoint_policy(&client).await?;
    ensure_validator_policy(&client).await?;
    ensure_training_policy(&client).await?;
    ensure_training_run(&client, &args).await?;

    println!("seeded_episode223_cs336_a1_demo=true");
    println!("base_url={}", args.base_url);
    println!("training_run_id={TRAINING_RUN_ID}");
    println!("network_id={NETWORK_ID}");
    println!("window_id={WINDOW_ID}");
    println!("artifact_bucket_uri={}", args.artifact_bucket_uri.as_str());
    Ok(())
}

#[derive(Clone, Debug)]
struct BootstrapArgs {
    base_url: String,
    desktop_client_id: String,
    device_name: String,
    client_version: String,
    artifact_bucket_uri: String,
}

impl BootstrapArgs {
    fn parse<I>(mut args: I) -> Result<Self>
    where
        I: Iterator<Item = String>,
    {
        let mut parsed = Self {
            base_url: DEFAULT_BASE_URL.to_string(),
            desktop_client_id: DEFAULT_DESKTOP_CLIENT_ID.to_string(),
            device_name: DEFAULT_DEVICE_NAME.to_string(),
            client_version: DEFAULT_CLIENT_VERSION.to_string(),
            artifact_bucket_uri: String::new(),
        };

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--base-url" => parsed.base_url = next_arg(&mut args, "--base-url")?,
                "--desktop-client-id" => {
                    parsed.desktop_client_id = next_arg(&mut args, "--desktop-client-id")?
                }
                "--device-name" => parsed.device_name = next_arg(&mut args, "--device-name")?,
                "--client-version" => {
                    parsed.client_version = next_arg(&mut args, "--client-version")?
                }
                "--artifact-bucket-uri" => {
                    parsed.artifact_bucket_uri = next_arg(&mut args, "--artifact-bucket-uri")?
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                other => bail!("unknown argument `{other}`"),
            }
        }

        if parsed.artifact_bucket_uri.trim().is_empty() {
            parsed.artifact_bucket_uri = default_artifact_bucket_uri(parsed.base_url.as_str());
        }
        Ok(parsed)
    }
}

fn next_arg<I>(args: &mut I, flag: &str) -> Result<String>
where
    I: Iterator<Item = String>,
{
    args.next()
        .with_context(|| format!("{flag} requires a value"))
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -p nexus-control --bin episode223-seed-cs336-a1-demo -- [options]\n\
         \n\
         options:\n\
           --base-url <url>              Nexus base URL (default: {DEFAULT_BASE_URL})\n\
           --artifact-bucket-uri <uri>   Training artifact bucket root\n\
           --desktop-client-id <id>      Session client id\n\
           --device-name <name>          Session device name\n\
           --client-version <version>    Session client version"
    );
}

fn default_artifact_bucket_uri(base_url: &str) -> String {
    if base_url.trim_end_matches('/') == "https://nexus.openagents.com" {
        PROD_ARTIFACT_BUCKET_URI.to_string()
    } else {
        LOCAL_ARTIFACT_BUCKET_URI.to_string()
    }
}

async fn mint_desktop_session(args: &BootstrapArgs) -> Result<DesktopSessionResponse> {
    let client = Client::builder()
        .build()
        .context("failed to build reqwest client")?;
    let endpoint = format!(
        "{}/api/session/desktop",
        args.base_url.trim_end_matches('/')
    );
    let response = client
        .post(endpoint)
        .json(&DesktopSessionCreateRequest {
            desktop_client_id: args.desktop_client_id.clone(),
            device_name: Some(args.device_name.clone()),
            bound_nostr_pubkey: None,
            client_version: Some(args.client_version.clone()),
        })
        .send()
        .await
        .context("failed to mint desktop session")?;
    let response = response.error_for_status().map_err(|error| {
        anyhow!(
            "failed to mint desktop session against {}: {error}",
            args.base_url
        )
    })?;
    response
        .json::<DesktopSessionResponse>()
        .await
        .context("failed to decode desktop session response")
}

async fn ensure_environment(
    client: &HttpKernelAuthorityClient,
    args: &BootstrapArgs,
) -> Result<()> {
    match client
        .get_compute_environment_package(
            PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF,
            Some(DEFAULT_ENVIRONMENT_VERSION),
        )
        .await
    {
        Ok(existing) => {
            validate_environment(&existing)?;
            println!(
                "environment_ref={} version={} status=present",
                existing.environment_ref, existing.version
            );
            Ok(())
        }
        Err(error) if is_not_found(&error) => {
            client
                .register_compute_environment_package(environment_request(args))
                .await
                .context("failed to register CS336 A1 demo environment")?;
            println!(
                "environment_ref={} version={} status=registered",
                PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF, DEFAULT_ENVIRONMENT_VERSION
            );
            Ok(())
        }
        Err(error) => Err(error).context("failed to query CS336 A1 demo environment"),
    }
}

async fn ensure_checkpoint_policy(client: &HttpKernelAuthorityClient) -> Result<()> {
    match client
        .get_compute_checkpoint_family_policy(CHECKPOINT_FAMILY, Some(DEFAULT_POLICY_VERSION))
        .await
    {
        Ok(existing) => {
            validate_checkpoint_policy(&existing)?;
            println!(
                "checkpoint_family={} version={} status=present",
                existing.checkpoint_family, existing.version
            );
            Ok(())
        }
        Err(error) if is_not_found(&error) => {
            client
                .register_compute_checkpoint_family_policy(checkpoint_policy_request())
                .await
                .context("failed to register checkpoint family policy")?;
            println!(
                "checkpoint_family={} version={} status=registered",
                CHECKPOINT_FAMILY, DEFAULT_POLICY_VERSION
            );
            Ok(())
        }
        Err(error) => Err(error).context("failed to query checkpoint family policy"),
    }
}

async fn ensure_validator_policy(client: &HttpKernelAuthorityClient) -> Result<()> {
    match client
        .get_compute_validator_policy(VALIDATOR_POLICY_REF, Some(DEFAULT_POLICY_VERSION))
        .await
    {
        Ok(existing) => {
            validate_validator_policy(&existing)?;
            println!(
                "validator_policy_ref={} version={} status=present",
                existing.policy_ref, existing.version
            );
            Ok(())
        }
        Err(error) if is_not_found(&error) => {
            client
                .register_compute_validator_policy(validator_policy_request())
                .await
                .context("failed to register validator policy")?;
            println!(
                "validator_policy_ref={} version={} status=registered",
                VALIDATOR_POLICY_REF, DEFAULT_POLICY_VERSION
            );
            Ok(())
        }
        Err(error) => Err(error).context("failed to query validator policy"),
    }
}

async fn ensure_training_policy(client: &HttpKernelAuthorityClient) -> Result<()> {
    match client
        .get_compute_training_policy(TRAINING_POLICY_REF, Some(DEFAULT_POLICY_VERSION))
        .await
    {
        Ok(existing) => {
            validate_training_policy(&existing)?;
            println!(
                "training_policy_ref={} version={} status=present",
                existing.training_policy_ref, existing.version
            );
            Ok(())
        }
        Err(error) if is_not_found(&error) => {
            client
                .register_compute_training_policy(training_policy_request())
                .await
                .context("failed to register training policy")?;
            println!(
                "training_policy_ref={} version={} status=registered",
                TRAINING_POLICY_REF, DEFAULT_POLICY_VERSION
            );
            Ok(())
        }
        Err(error) => Err(error).context("failed to query training policy"),
    }
}

async fn ensure_training_run(
    client: &HttpKernelAuthorityClient,
    args: &BootstrapArgs,
) -> Result<()> {
    match client.get_compute_training_run(TRAINING_RUN_ID).await {
        Ok(existing) => {
            validate_training_run(&existing, args)?;
            println!(
                "training_run_id={} status={} worker_count=2 state=present",
                existing.training_run_id,
                existing.status.label()
            );
            Ok(())
        }
        Err(error) if is_not_found(&error) => {
            client
                .create_compute_training_run(training_run_request(args))
                .await
                .context("failed to create CS336 A1 demo training run")?;
            println!(
                "training_run_id={} status=registered worker_count=2",
                TRAINING_RUN_ID
            );
            Ok(())
        }
        Err(error) => Err(error).context("failed to query CS336 A1 demo training run"),
    }
}

fn environment_request(args: &BootstrapArgs) -> RegisterComputeEnvironmentPackageRequest {
    let created_at_ms = now_unix_ms();
    RegisterComputeEnvironmentPackageRequest {
        idempotency_key: "episode223.cs336_a1_demo.environment.v1".to_string(),
        trace: TraceContext::default(),
        policy: kernel_policy(),
        package: ComputeEnvironmentPackage {
            environment_ref: PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string(),
            version: DEFAULT_ENVIRONMENT_VERSION.to_string(),
            family: "training".to_string(),
            display_name: "Psion CS336 A1 Demo".to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms,
            updated_at_ms: created_at_ms,
            status: ComputeEnvironmentPackageStatus::Active,
            description: Some(
                "Bounded CS336 assignment 1 demo environment for Episode 223".to_string(),
            ),
            package_digest: Some(format!(
                "sha256:{}:{}",
                PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF, DEFAULT_ENVIRONMENT_VERSION
            )),
            dataset_bindings: vec![ComputeEnvironmentDatasetBinding {
                dataset_ref: "dataset://cs336/assignment1/tinystories-demo".to_string(),
                split_ref: Some("train".to_string()),
                mount_path: Some("/datasets/cs336/assignment1/tinystories-demo".to_string()),
                integrity_ref: Some(
                    "sha256:dataset.cs336.assignment1.tinystories-demo".to_string(),
                ),
                access_policy_ref: Some("policy://dataset/cs336/assignment1/demo".to_string()),
                required: true,
                metadata: json!({"format": "jsonl", "episode": 223}),
            }],
            harness: Some(ComputeEnvironmentHarness {
                harness_ref: "harness://psionic/cs336/a1-demo".to_string(),
                runtime_family: "psionic-train".to_string(),
                entrypoint: Some("psionic-train".to_string()),
                args: vec!["--lane".to_string(), "cs336_a1_demo".to_string()],
                sandbox_profile_ref: Some("sandbox://training/bounded".to_string()),
                evaluator_policy_ref: Some(VALIDATOR_POLICY_REF.to_string()),
                time_budget_ms: Some(3_600_000),
                metadata: json!({
                    "lane": "cs336_a1_demo",
                    "artifact_bucket_uri": args.artifact_bucket_uri,
                }),
            }),
            rubric_bindings: vec![ComputeEnvironmentRubricBinding {
                rubric_ref: "rubric://cs336/assignment1/demo".to_string(),
                score_type: Some("completion".to_string()),
                pass_threshold_bps: None,
                metadata: json!({"bounded": true}),
            }],
            expected_artifacts: vec![
                ComputeEnvironmentArtifactExpectation {
                    artifact_kind: "training_manifest".to_string(),
                    artifact_ref: Some("artifact://cs336/a1-demo/manifest".to_string()),
                    required: true,
                    verification_policy_ref: None,
                    metadata: json!({"schema": "psionic_train_manifest.v1"}),
                },
                ComputeEnvironmentArtifactExpectation {
                    artifact_kind: "training_closeout".to_string(),
                    artifact_ref: Some("artifact://cs336/a1-demo/closeout".to_string()),
                    required: false,
                    verification_policy_ref: None,
                    metadata: json!({"schema": "psionic_train_closeout.v1"}),
                },
            ],
            policy_refs: vec![
                TRAINING_POLICY_REF.to_string(),
                VALIDATOR_POLICY_REF.to_string(),
            ],
            metadata: json!({
                "lane": "cs336_a1_demo",
                "episode": 223,
                "role_goal": "mac_and_linux_both_do_the_homework",
            }),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn checkpoint_policy_request() -> RegisterComputeCheckpointFamilyPolicyRequest {
    let created_at_ms = now_unix_ms();
    RegisterComputeCheckpointFamilyPolicyRequest {
        idempotency_key: "episode223.cs336_a1_demo.checkpoint_policy.v1".to_string(),
        trace: TraceContext::default(),
        policy: kernel_policy(),
        policy_record: ComputeCheckpointFamilyPolicy {
            checkpoint_family: CHECKPOINT_FAMILY.to_string(),
            version: DEFAULT_POLICY_VERSION.to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms,
            updated_at_ms: created_at_ms,
            status: ComputeRegistryStatus::Active,
            description: Some("Episode 223 CS336 A1 demo checkpoint policy".to_string()),
            source_family: Some("bounded-demo".to_string()),
            default_recovery_posture: Some("warm-resume".to_string()),
            allowed_environment_refs: vec![
                PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string(),
            ],
            validator_policy_ref: Some(VALIDATOR_POLICY_REF.to_string()),
            retention_policy_ref: Some("policy://retention/cs336/a1-demo".to_string()),
            metadata: json!({"episode": 223, "lane": "cs336_a1_demo"}),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn validator_policy_request() -> RegisterComputeValidatorPolicyRequest {
    let created_at_ms = now_unix_ms();
    RegisterComputeValidatorPolicyRequest {
        idempotency_key: "episode223.cs336_a1_demo.validator_policy.v1".to_string(),
        trace: TraceContext::default(),
        policy: kernel_policy(),
        policy_record: ComputeValidatorPolicy {
            policy_ref: VALIDATOR_POLICY_REF.to_string(),
            version: DEFAULT_POLICY_VERSION.to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms,
            updated_at_ms: created_at_ms,
            status: ComputeRegistryStatus::Active,
            validator_pool_ref: "validator-pool.mvp".to_string(),
            minimum_validator_count: Some(1),
            challenge_window_ms: Some(60_000),
            required_proof_posture: Some(ComputeProofPosture::ChallengeEligible),
            benchmark_package_refs: Vec::new(),
            metadata: json!({"episode": 223, "mode": "bounded_demo"}),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn training_policy_request() -> RegisterComputeTrainingPolicyRequest {
    let created_at_ms = now_unix_ms();
    RegisterComputeTrainingPolicyRequest {
        idempotency_key: "episode223.cs336_a1_demo.training_policy.v1".to_string(),
        trace: TraceContext::default(),
        policy: kernel_policy(),
        training_policy: ComputeTrainingPolicy {
            training_policy_ref: TRAINING_POLICY_REF.to_string(),
            version: DEFAULT_POLICY_VERSION.to_string(),
            owner_id: "openagents".to_string(),
            created_at_ms,
            updated_at_ms: created_at_ms,
            status: ComputeRegistryStatus::Active,
            environment_refs: vec![PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string()],
            checkpoint_family: CHECKPOINT_FAMILY.to_string(),
            validator_policy_ref: VALIDATOR_POLICY_REF.to_string(),
            benchmark_package_refs: Vec::new(),
            stage_policy_refs: vec![
                "policy://training/cs336/a1-demo/worker".to_string(),
                "policy://training/cs336/a1-demo/checkpoint".to_string(),
            ],
            metadata: training_run_definition_metadata_value(),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn training_run_request(
    args: &BootstrapArgs,
) -> openagents_kernel_core::authority::CreateComputeTrainingRunRequest {
    let created_at_ms = now_unix_ms();
    openagents_kernel_core::authority::CreateComputeTrainingRunRequest {
        idempotency_key: "episode223.cs336_a1_demo.training_run.v1".to_string(),
        trace: TraceContext::default(),
        policy: kernel_policy(),
        training_run: ComputeTrainingRun {
            training_run_id: TRAINING_RUN_ID.to_string(),
            training_policy_ref: TRAINING_POLICY_REF.to_string(),
            environment_binding: ComputeEnvironmentBinding {
                environment_ref: PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF.to_string(),
                environment_version: Some(DEFAULT_ENVIRONMENT_VERSION.to_string()),
                dataset_ref: Some("dataset://cs336/assignment1/tinystories-demo".to_string()),
                rubric_ref: Some("rubric://cs336/assignment1/demo".to_string()),
                evaluator_policy_ref: Some(VALIDATOR_POLICY_REF.to_string()),
            },
            checkpoint_binding: ComputeCheckpointBinding {
                checkpoint_family: CHECKPOINT_FAMILY.to_string(),
                latest_checkpoint_ref: Some(BASE_CHECKPOINT_REF.to_string()),
                recovery_posture: Some("warm-resume".to_string()),
            },
            validator_policy_ref: VALIDATOR_POLICY_REF.to_string(),
            work_class: ComputeTrainingWorkClass::SmallModelLocalTraining,
            replica_type: ComputeTrainingReplicaType::SingleNode,
            benchmark_package_refs: Vec::new(),
            product_id: Some("psionic.training.cs336_a1_demo".to_string()),
            capacity_lot_id: None,
            instrument_id: None,
            delivery_proof_id: None,
            model_ref: Some("model://psion/reference".to_string()),
            source_ref: Some("artifact://cs336/a1-demo/input".to_string()),
            rollout_verification_eval_run_ids: Vec::new(),
            created_at_ms,
            started_at_ms: Some(created_at_ms),
            finalized_at_ms: None,
            expected_step_count: Some(4),
            completed_step_count: None,
            status: ComputeTrainingRunStatus::Running,
            final_checkpoint_ref: None,
            promotion_checkpoint_ref: None,
            summary: None,
            metadata: training_run_metadata(args),
        },
        evidence: Vec::new(),
        hints: ReceiptHints::default(),
    }
}

fn training_run_metadata(args: &BootstrapArgs) -> Value {
    json!({
        "display_name": "CS336 A1 Demo",
        "pylon_training_scheduler": {
            "network_id": NETWORK_ID,
            "artifact_bucket_uri": args.artifact_bucket_uri,
            "worker_count": 2,
            "validator_count": 0,
            "recovery_source_count": 0,
            "initial_window_id": WINDOW_ID,
            "checkpoint_ref": BASE_CHECKPOINT_REF,
        }
    })
}

fn training_run_definition_metadata_value() -> Value {
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
            window_ref_family: Some("window.family.diloco_round".to_string()),
            manifest_ref_family: Some("manifest.family.psionic_train".to_string()),
            trn_ref_family: Some("trn.family.diloco".to_string()),
            closeout_ref_family: Some("closeout.family.accepted_training".to_string()),
        })
        .expect("run definition metadata"),
    })
}

fn validate_environment(environment: &ComputeEnvironmentPackage) -> Result<()> {
    if environment.environment_ref != PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF {
        bail!(
            "unexpected environment ref `{}` for Episode 223 bootstrap",
            environment.environment_ref
        );
    }
    if environment.family != "training" {
        bail!("Episode 223 environment must stay in the training family");
    }
    Ok(())
}

fn validate_checkpoint_policy(policy: &ComputeCheckpointFamilyPolicy) -> Result<()> {
    if !policy
        .allowed_environment_refs
        .iter()
        .any(|value| value == PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF)
    {
        bail!("checkpoint policy is not bound to the CS336 A1 demo environment");
    }
    Ok(())
}

fn validate_validator_policy(policy: &ComputeValidatorPolicy) -> Result<()> {
    if policy.policy_ref != VALIDATOR_POLICY_REF {
        bail!("unexpected validator policy ref `{}`", policy.policy_ref);
    }
    Ok(())
}

fn validate_training_policy(policy: &ComputeTrainingPolicy) -> Result<()> {
    if policy.training_policy_ref != TRAINING_POLICY_REF {
        bail!(
            "unexpected training policy ref `{}` for Episode 223 bootstrap",
            policy.training_policy_ref
        );
    }
    if !policy
        .environment_refs
        .iter()
        .any(|value| value == PYLON_TRAINING_CS336_A1_DEMO_ENVIRONMENT_REF)
    {
        bail!("training policy is not bound to the CS336 A1 demo environment");
    }
    Ok(())
}

fn validate_training_run(run: &ComputeTrainingRun, args: &BootstrapArgs) -> Result<()> {
    if run.training_run_id != TRAINING_RUN_ID {
        bail!("unexpected training run id `{}`", run.training_run_id);
    }
    if run.training_policy_ref != TRAINING_POLICY_REF {
        bail!("training run is not bound to the CS336 A1 demo policy");
    }
    let Some(display_name) = run
        .metadata
        .get("display_name")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        bail!("training run is missing display_name metadata");
    };
    if display_name != "CS336 A1 Demo" {
        bail!("training run display_name changed to `{display_name}`");
    }
    let scheduler = run
        .metadata
        .get("pylon_training_scheduler")
        .cloned()
        .context("training run is missing pylon_training_scheduler metadata")?;
    let worker_count = scheduler
        .get("worker_count")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if worker_count != 2 {
        bail!("training run worker_count must stay at 2 for Episode 223");
    }
    let artifact_bucket_uri = scheduler
        .get("artifact_bucket_uri")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if artifact_bucket_uri != args.artifact_bucket_uri {
        bail!(
            "training run artifact_bucket_uri `{artifact_bucket_uri}` does not match requested `{}`",
            args.artifact_bucket_uri
        );
    }
    Ok(())
}

fn kernel_policy() -> PolicyContext {
    PolicyContext::default()
}

fn is_not_found(error: &anyhow::Error) -> bool {
    error.to_string().contains("status=404")
}

fn now_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(i64::MAX)
}

fn ensure_rustls_crypto_provider() -> Result<()> {
    if rustls::crypto::CryptoProvider::get_default().is_some() {
        return Ok(());
    }

    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|error| anyhow!("failed to install rustls crypto provider: {error:?}"))
}
