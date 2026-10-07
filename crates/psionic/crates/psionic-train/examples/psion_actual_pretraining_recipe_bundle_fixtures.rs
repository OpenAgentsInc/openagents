use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_train::{
    PsionActualPretrainingArtifactRef, PsionActualPretrainingContinuationTarget,
    PsionActualPretrainingCredentialSource, PsionActualPretrainingRecipeBundle,
    PsionActualPretrainingStageSchedule, PsionActualPretrainingStorageTier,
    PsionActualPretrainingTopologyStorageBundle, PSION_ACTUAL_PRETRAINING_LANE_ID,
    PSION_ACTUAL_PRETRAINING_RECIPE_BUNDLE_SCHEMA_VERSION, PSION_ACTUAL_PRETRAINING_RECIPE_ID,
    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID,
    PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_SCHEMA_VERSION,
};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let trusted_cluster_run: serde_json::Value = read_json(
        &root.join("fixtures/psion/trusted_cluster/psion_trusted_cluster_run_bundle_v1.json"),
    )?;
    let tokenized_corpus: serde_json::Value = read_json(
        &root.join("fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"),
    )?;
    let topology_contract: serde_json::Value = read_json(
        &root.join("fixtures/psion/trusted_cluster/psion_trusted_cluster_topology_contract_v1.json"),
    )?;
    let model_descriptor_path =
        root.join("fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json");
    let sampling_policy_path =
        root.join("fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json");
    let lane_spec_path = root.join("fixtures/psion/pretrain/psion_actual_pretraining_lane_spec_v1.json");
    let reasoning_sft_path = root.join("fixtures/psion/sft/psion_reasoning_sft_run_bundle_v1.json");
    let plugin_manifest_path = root.join(
        "fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_stage_manifest.json",
    );
    let plugin_run_bundle_path = root.join(
        "fixtures/psion/plugins/training/psion_plugin_conditioned_sft_v1/psion_plugin_conditioned_sft_run_bundle.json",
    );
    let continuation_eval_pack_path = root.join(
        "fixtures/psion/pretrain/psion_actual_pretraining_continuation_eval_benchmark_pack_v1.json",
    );

    let recipe_bundle = PsionActualPretrainingRecipeBundle {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_BUNDLE_SCHEMA_VERSION),
        recipe_id: String::from(PSION_ACTUAL_PRETRAINING_RECIPE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        lane_spec: artifact_ref(&root, &lane_spec_path)?,
        model_id: json_string(
            &trusted_cluster_run,
            &["pretrain_stage_receipt", "model_id"],
            "trusted_cluster_run_bundle.model_id",
        )?,
        model_descriptor: artifact_ref(&root, &model_descriptor_path)?,
        model_descriptor_digest: json_string(
            &trusted_cluster_run,
            &["pretrain_stage_receipt", "model_descriptor_digest"],
            "trusted_cluster_run_bundle.model_descriptor_digest",
        )?,
        tokenizer_id: json_string(
            &tokenized_corpus,
            &["tokenizer_id"],
            "tokenized_corpus_manifest.tokenizer_id",
        )?,
        tokenizer_version: json_string(
            &tokenized_corpus,
            &["tokenizer_version"],
            "tokenized_corpus_manifest.tokenizer_version",
        )?,
        tokenizer_digest: json_string(
            &tokenized_corpus,
            &["tokenizer", "tokenizer_digest"],
            "tokenized_corpus_manifest.tokenizer.tokenizer_digest",
        )?,
        tokenized_corpus_manifest: artifact_ref(
            &root,
            &root.join("fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"),
        )?,
        dataset_identity: json_string(
            &trusted_cluster_run,
            &["pretrain_stage_receipt", "dataset_identity"],
            "trusted_cluster_run_bundle.dataset_identity",
        )?,
        sampling_policy_id: json_string(
            &trusted_cluster_run,
            &["pretrain_stage_receipt", "sampling_policy_id"],
            "trusted_cluster_run_bundle.sampling_policy_id",
        )?,
        sampling_policy_version: json_string(
            &trusted_cluster_run,
            &["pretrain_stage_receipt", "sampling_policy_version"],
            "trusted_cluster_run_bundle.sampling_policy_version",
        )?,
        sampling_policy_manifest: artifact_ref(&root, &sampling_policy_path)?,
        stage_schedule: PsionActualPretrainingStageSchedule {
            base_stage_kinds: vec![String::from("pretrain")],
            train_token_budget: json_u64(
                &trusted_cluster_run,
                &["observability_receipt", "throughput", "train_tokens_processed"],
                "trusted_cluster_run_bundle.train_tokens_processed",
            )?,
            validation_token_budget: json_u64(
                &trusted_cluster_run,
                &["observability_receipt", "throughput", "validation_tokens_processed"],
                "trusted_cluster_run_bundle.validation_tokens_processed",
            )?,
            held_out_token_budget: json_u64(
                &trusted_cluster_run,
                &["observability_receipt", "throughput", "held_out_tokens_scored"],
                "trusted_cluster_run_bundle.held_out_tokens_scored",
            )?,
            optimizer_steps: json_u64(
                &trusted_cluster_run,
                &["observability_receipt", "throughput", "optimizer_steps_completed"],
                "trusted_cluster_run_bundle.optimizer_steps_completed",
            )?,
            max_context_tokens: json_u64(
                &trusted_cluster_run,
                &[
                    "pretrain_stage_receipt",
                    "objective_config",
                    "max_context_tokens",
                ],
                "trusted_cluster_run_bundle.max_context_tokens",
            )?,
        },
        continuation_target: PsionActualPretrainingContinuationTarget {
            stage_path: vec![
                String::from("pretrain"),
                String::from("general_sft"),
                String::from("agentic_sft"),
            ],
            reasoning_sft_run_bundle: artifact_ref(&root, &reasoning_sft_path)?,
            plugin_conditioned_stage_manifest: artifact_ref(&root, &plugin_manifest_path)?,
            plugin_conditioned_run_bundle: artifact_ref(&root, &plugin_run_bundle_path)?,
            continuation_eval_pack: artifact_ref(&root, &continuation_eval_pack_path)?,
            claim_boundary: String::from(
                "The actual pretraining lane declares a bounded continuation path through the existing reasoning `general_sft` bundle into the bounded plugin-conditioned `agentic_sft` lane and carries one continuation-stage eval pack for later review without implying cluster-scale plugin-conditioned training closure.",
            ),
        },
        summary: String::from(
            "The canonical actual-pretraining recipe binds the broader-pretraining trusted-cluster receipt to one frozen model, tokenizer, tokenized corpus, mixture policy, fixed budget, and bounded continuation path.",
        ),
    };
    recipe_bundle.validate()?;

    let topology_storage_bundle = PsionActualPretrainingTopologyStorageBundle {
        schema_version: String::from(
            PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_SCHEMA_VERSION,
        ),
        bundle_id: String::from(PSION_ACTUAL_PRETRAINING_TOPOLOGY_STORAGE_BUNDLE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        topology_contract: artifact_ref(
            &root,
            &root.join(
                "fixtures/psion/trusted_cluster/psion_trusted_cluster_topology_contract_v1.json",
            ),
        )?,
        supported_topology_label: json_string(
            &topology_contract,
            &["supported_topology_label"],
            "topology_contract.supported_topology_label",
        )?,
        required_backend: json_string(
            &topology_contract,
            &["required_backend"],
            "topology_contract.required_backend",
        )?,
        required_worker_count: json_u64(
            &topology_contract,
            &["required_worker_count"],
            "topology_contract.required_worker_count",
        )?,
        placement_shape: String::from("tensor_parallel(world_size=4,ranks_per_node=1)"),
        remote_run_root_template: String::from(
            "${PSION_ACTUAL_PRETRAINING_BUCKET_URL}/psion_actual_pretraining_runs/<run_id>",
        ),
        remote_checkpoint_root_template: String::from(
            "${PSION_ACTUAL_PRETRAINING_BUCKET_URL}/psion_actual_pretraining_runs/<run_id>/checkpoints",
        ),
        remote_manifest_root_template: String::from(
            "${PSION_ACTUAL_PRETRAINING_BUCKET_URL}/psion_actual_pretraining_runs/<run_id>/manifests",
        ),
        remote_log_root_template: String::from(
            "${PSION_ACTUAL_PRETRAINING_BUCKET_URL}/psion_actual_pretraining_runs/<run_id>/logs",
        ),
        storage_tiers: vec![
            PsionActualPretrainingStorageTier {
                prefix: String::from("checkpoints/"),
                durability_class: String::from("durable"),
                detail: String::from(
                    "Actual-lane checkpoints remain durable authority for resume, backup, and closeout.",
                ),
            },
            PsionActualPretrainingStorageTier {
                prefix: String::from("manifests/"),
                durability_class: String::from("durable"),
                detail: String::from(
                    "Launch, evidence, and replay manifests remain durable retained artifacts.",
                ),
            },
            PsionActualPretrainingStorageTier {
                prefix: String::from("logs/"),
                durability_class: String::from("transient"),
                detail: String::from(
                    "Logs remain transient and are retained only as long as the later evidence contract allows.",
                ),
            },
        ],
        credential_sources: vec![
            PsionActualPretrainingCredentialSource {
                kind: String::from("environment_variable"),
                source_name: String::from("PSION_ACTUAL_PRETRAINING_GCP_PROJECT_ID"),
                purpose: String::from("select the admitted cloud project for remote artifact storage"),
                retained_redaction: String::from("record only the env-var name and resolved project digest"),
            },
            PsionActualPretrainingCredentialSource {
                kind: String::from("environment_variable"),
                source_name: String::from("PSION_ACTUAL_PRETRAINING_BUCKET_URL"),
                purpose: String::from("select the admitted remote artifact and checkpoint bucket root"),
                retained_redaction: String::from("record only the env-var name and selected bucket digest"),
            },
            PsionActualPretrainingCredentialSource {
                kind: String::from("secret_file_env"),
                source_name: String::from("GOOGLE_APPLICATION_CREDENTIALS"),
                purpose: String::from("authenticate retained storage uploads without embedding raw credentials in manifests"),
                retained_redaction: String::from("record only the env-var name and credential-file digest"),
            },
        ],
        summary: String::from(
            "The canonical actual-pretraining topology and storage bundle admits one homogeneous four-node H100 tensor-parallel mesh plus env-declared durable storage roots and credential sources without embedding secrets.",
        ),
    };
    topology_storage_bundle.validate()?;

    fs::write(
        fixtures_dir.join("psion_actual_pretraining_recipe_bundle_v1.json"),
        serde_json::to_string_pretty(&recipe_bundle)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_actual_pretraining_topology_storage_bundle_v1.json"),
        serde_json::to_string_pretty(&topology_storage_bundle)?,
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

fn read_json(path: &Path) -> Result<serde_json::Value, Box<dyn Error>> {
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn artifact_ref(
    root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    let relative = path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(PsionActualPretrainingArtifactRef {
        path: relative,
        sha256: file_sha256(path)?,
    })
}

fn file_sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let mut digest = Sha256::new();
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn json_string(
    value: &serde_json::Value,
    path: &[&str],
    label: &str,
) -> Result<String, Box<dyn Error>> {
    let mut current = value;
    for segment in path {
        current = current
            .get(segment)
            .ok_or_else(|| format!("missing `{label}` segment `{segment}`"))?;
    }
    current
        .as_str()
        .map(String::from)
        .ok_or_else(|| format!("`{label}` must be a string").into())
}

fn json_u64(
    value: &serde_json::Value,
    path: &[&str],
    label: &str,
) -> Result<u64, Box<dyn Error>> {
    let mut current = value;
    for segment in path {
        current = current
            .get(segment)
            .ok_or_else(|| format!("missing `{label}` segment `{segment}`"))?;
    }
    current
        .as_u64()
        .ok_or_else(|| format!("`{label}` must be a u64").into())
}
