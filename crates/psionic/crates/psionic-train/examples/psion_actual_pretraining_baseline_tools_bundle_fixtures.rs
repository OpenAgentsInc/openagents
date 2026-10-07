use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use psionic_data::{
    PsionTokenizedCorpusManifest, PsionTokenizerArtifactBundle, PsionTokenizerTrainingManifest,
};
use psionic_models::PsionCompactDecoderDescriptor;
use psionic_train::{
    PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_ID,
    PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_SCHEMA_VERSION,
    PSION_ACTUAL_PRETRAINING_LANE_ID, PsionActualPretrainingArtifactRef,
    PsionActualPretrainingBaselineToolsBundle, PsionActualPretrainingBoundedAblationConfig,
    PsionActualPretrainingBringupTrainer, PsionActualPretrainingResourceAccountingRow,
    PsionActualPretrainingTokenizerReproducibilityBinding, PsionPretrainLossNormalization,
    PsionPretrainObjectiveConfig, PsionPretrainObjectiveKind, PsionPretrainStageConfig,
    PsionSamplingPolicyManifest, stable_baseline_tools_bundle_digest,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let lane_spec_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_lane_spec_v1.json");
    let recipe_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_recipe_bundle_v1.json");
    let scaling_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_scaling_bundle_v1.json");
    let data_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_data_bundle_v1.json");
    let systems_bundle_path =
        root.join("fixtures/psion/pretrain/psion_actual_pretraining_systems_bundle_v1.json");
    let tokenizer_training_manifest_path =
        root.join("fixtures/psion/tokenizer/psion_tokenizer_training_manifest_v1.json");
    let tokenizer_artifact_bundle_path =
        root.join("fixtures/psion/tokenizer/psion_tokenizer_artifact_bundle_v1.json");
    let tokenized_corpus_manifest_path =
        root.join("fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json");
    let sampling_policy_path =
        root.join("fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json");
    let internal_descriptor_path =
        root.join("fixtures/psion/models/psion_compact_decoder_internal_descriptor_v1.json");
    let pilot_descriptor_path =
        root.join("fixtures/psion/models/psion_compact_decoder_pilot_descriptor_v1.json");

    let recipe_bundle: serde_json::Value = load_json(&recipe_bundle_path)?;
    let systems_bundle: serde_json::Value = load_json(&systems_bundle_path)?;
    let tokenizer_training_manifest: PsionTokenizerTrainingManifest =
        load_json(&tokenizer_training_manifest_path)?;
    let tokenizer_artifact_bundle: PsionTokenizerArtifactBundle =
        load_json(&tokenizer_artifact_bundle_path)?;
    let tokenized_corpus_manifest: PsionTokenizedCorpusManifest =
        load_json(&tokenized_corpus_manifest_path)?;
    let sampling_policy: PsionSamplingPolicyManifest = load_json(&sampling_policy_path)?;
    let internal_descriptor: PsionCompactDecoderDescriptor = load_json(&internal_descriptor_path)?;
    let pilot_descriptor: PsionCompactDecoderDescriptor = load_json(&pilot_descriptor_path)?;

    let actual_stage_config = PsionPretrainStageConfig::new(
        "psion-actual-pretraining-bringup",
        "psion-actual-pretraining-stage-pretrain-bringup",
        objective_config(&internal_descriptor, &tokenized_corpus_manifest),
        &internal_descriptor,
        &tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let actual_stage_config_path =
        fixtures_dir.join("psion_actual_pretraining_bringup_stage_config_v1.json");
    write_json(&actual_stage_config_path, &actual_stage_config)?;

    let pilot_stage_config = PsionPretrainStageConfig::new(
        "psion-actual-pretraining-pilot32m-ablation",
        "psion-actual-pretraining-stage-pretrain-pilot32m-ablation",
        objective_config(&pilot_descriptor, &tokenized_corpus_manifest),
        &pilot_descriptor,
        &tokenized_corpus_manifest,
        &sampling_policy,
    )?;
    let pilot_stage_config_path =
        fixtures_dir.join("psion_actual_pretraining_pilot32m_ablation_stage_config_v1.json");
    write_json(&pilot_stage_config_path, &pilot_stage_config)?;

    let actual_train_token_budget =
        u64_at(&recipe_bundle, &["stage_schedule", "train_token_budget"])?;
    let actual_validation_token_budget = u64_at(
        &recipe_bundle,
        &["stage_schedule", "validation_token_budget"],
    )?;
    let actual_held_out_token_budget =
        u64_at(&recipe_bundle, &["stage_schedule", "held_out_token_budget"])?;
    let actual_optimizer_steps = u64_at(&recipe_bundle, &["stage_schedule", "optimizer_steps"])?;
    let actual_tokens_per_step = actual_train_token_budget / actual_optimizer_steps;
    let actual_checkpoint_total_bytes = u64_at(
        &systems_bundle,
        &["memory_qualification", "checkpoint_total_bytes"],
    )?;
    let actual_optimizer_state_bytes = u64_at(
        &systems_bundle,
        &["memory_qualification", "optimizer_state_bytes"],
    )?;
    let actual_min_free_memory_bytes = u64_at(
        &systems_bundle,
        &["memory_qualification", "min_per_worker_free_memory_bytes"],
    )?;
    let actual_activation_headroom_bytes = actual_min_free_memory_bytes
        .saturating_sub(actual_checkpoint_total_bytes)
        .saturating_sub(actual_optimizer_state_bytes);
    let actual_mean_tokens_per_second = u64_at(
        &systems_bundle,
        &["throughput_baselines", "0", "mean_tokens_per_second"],
    )?;

    let pilot_train_token_budget = 8_388_608;
    let pilot_validation_token_budget = pilot_train_token_budget / 32;
    let pilot_optimizer_steps = 256;
    let pilot_tokens_per_step = pilot_train_token_budget / pilot_optimizer_steps;
    let pilot_parameter_ratio = pilot_descriptor.parameter_count_estimate as f64
        / internal_descriptor.parameter_count_estimate as f64;
    let pilot_checkpoint_total_bytes =
        round_scaled(actual_checkpoint_total_bytes, pilot_parameter_ratio);
    let pilot_optimizer_state_bytes =
        round_scaled(actual_optimizer_state_bytes, pilot_parameter_ratio);
    let pilot_activation_headroom_bytes = actual_min_free_memory_bytes
        .saturating_sub(pilot_checkpoint_total_bytes)
        .saturating_sub(pilot_optimizer_state_bytes);
    let pilot_mean_tokens_per_second = round_scaled(
        actual_mean_tokens_per_second,
        parameter_speedup_ratio(
            internal_descriptor.parameter_count_estimate,
            pilot_descriptor.parameter_count_estimate,
        ),
    );

    let tokenizer_only_source_ids = tokenizer_training_manifest
        .exposure_report
        .iter()
        .filter(|row| row.tokenizer_only_exposure)
        .map(|row| row.source_id.clone())
        .collect::<Vec<_>>();
    let model_training_source_ids = tokenizer_training_manifest
        .exposure_report
        .iter()
        .filter(|row| row.model_training_exposed)
        .map(|row| row.source_id.clone())
        .collect::<Vec<_>>();
    let held_out_source_ids = tokenized_corpus_manifest
        .shards
        .iter()
        .filter(|shard| shard.split_name == "held_out")
        .flat_map(|shard| shard.source_lineage.iter().map(|row| row.source_id.clone()))
        .collect::<Vec<_>>();

    let mut bundle = PsionActualPretrainingBaselineToolsBundle {
        schema_version: String::from(PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_SCHEMA_VERSION),
        baseline_tools_bundle_id: String::from(PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE_ID),
        lane_id: String::from(PSION_ACTUAL_PRETRAINING_LANE_ID),
        lane_spec: artifact_ref(&root, &lane_spec_path)?,
        recipe_bundle: artifact_ref(&root, &recipe_bundle_path)?,
        scaling_bundle: artifact_ref(&root, &scaling_bundle_path)?,
        data_bundle: artifact_ref(&root, &data_bundle_path)?,
        systems_bundle: artifact_ref(&root, &systems_bundle_path)?,
        bringup_trainer: PsionActualPretrainingBringupTrainer {
            trainer_id: String::from("psion_actual_pretraining_bringup_trainer_v1"),
            stage_program_id: String::from("psion_pretrain_stage"),
            trainer_entry_surface: String::from("crates/psionic-train/src/psion_pretrain_stage.rs"),
            stage_config: artifact_ref(&root, &actual_stage_config_path)?,
            model_descriptor: artifact_ref(&root, &internal_descriptor_path)?,
            model_id: internal_descriptor.model.model_id.clone(),
            dataset_identity: actual_stage_config.dataset_identity.clone(),
            sampling_policy_id: actual_stage_config.sampling_policy_id.clone(),
            sampling_policy_version: actual_stage_config.sampling_policy_version.clone(),
            tokenizer_binding_digest: actual_stage_config.tokenizer_binding_digest.clone(),
            max_context_tokens: actual_stage_config.objective_config.max_context_tokens as u64,
            detail: String::from(
                "The actual lane now binds one honest pretrain-stage bring-up surface directly to the frozen internal 128M recipe instead of leaving correctness and launch-shape checks buried in the larger trusted-cluster anchor.",
            ),
        },
        tokenizer_reproducibility: PsionActualPretrainingTokenizerReproducibilityBinding {
            tokenizer_training_manifest: artifact_ref(&root, &tokenizer_training_manifest_path)?,
            tokenizer_artifact_bundle: artifact_ref(&root, &tokenizer_artifact_bundle_path)?,
            tokenized_corpus_manifest: artifact_ref(&root, &tokenized_corpus_manifest_path)?,
            tokenizer_id: tokenizer_training_manifest.tokenizer_id.clone(),
            tokenizer_version: tokenizer_training_manifest.tokenizer_version.clone(),
            tokenizer_digest: tokenizer_artifact_bundle.tokenizer.tokenizer_digest.clone(),
            tokenizer_binding_digest: internal_descriptor.tokenizer_binding.stable_digest(),
            tokenizer_only_source_ids,
            model_training_source_ids,
            held_out_source_ids,
            detail: String::from(
                "Tokenizer reproducibility stays bound to the canonical tokenizer training manifest, artifact bundle, and tokenized corpus manifest so actual-lane bring-up and ablations cannot silently drift vocab or held-out exposure.",
            ),
        },
        resource_accounting_rows: vec![
            PsionActualPretrainingResourceAccountingRow {
                row_id: String::from("psion_actual_pretraining_internal128m_accounting"),
                scope_kind: String::from("actual_lane"),
                config_binding_id: String::from("psion_actual_pretraining_internal128m_anchor"),
                model_id: internal_descriptor.model.model_id.clone(),
                size_anchor: size_anchor_label(&internal_descriptor),
                parameter_count_estimate: internal_descriptor.parameter_count_estimate,
                train_token_budget: actual_train_token_budget,
                validation_token_budget: actual_validation_token_budget,
                held_out_token_budget: actual_held_out_token_budget,
                optimizer_steps: actual_optimizer_steps,
                tokens_per_step: actual_tokens_per_step,
                max_context_tokens: internal_descriptor.config.max_context as u64,
                checkpoint_total_bytes: actual_checkpoint_total_bytes,
                optimizer_state_bytes: actual_optimizer_state_bytes,
                activation_headroom_bytes: actual_activation_headroom_bytes,
                expected_mean_tokens_per_second: actual_mean_tokens_per_second,
                detail: String::from(
                    "Resource accounting exposes the canonical 128M lane shape in one smaller operator-readable table derived from the frozen recipe and systems bundle instead of forcing every operator check to re-open the full anchor receipts.",
                ),
            },
            PsionActualPretrainingResourceAccountingRow {
                row_id: String::from("psion_actual_pretraining_pilot32m_ablation_accounting"),
                scope_kind: String::from("bounded_ablation"),
                config_binding_id: String::from(
                    "psion_actual_pretraining_pilot32m_replay_ablation",
                ),
                model_id: pilot_descriptor.model.model_id.clone(),
                size_anchor: size_anchor_label(&pilot_descriptor),
                parameter_count_estimate: pilot_descriptor.parameter_count_estimate,
                train_token_budget: pilot_train_token_budget,
                validation_token_budget: pilot_validation_token_budget,
                held_out_token_budget: pilot_validation_token_budget / 4,
                optimizer_steps: pilot_optimizer_steps,
                tokens_per_step: pilot_tokens_per_step,
                max_context_tokens: pilot_descriptor.config.max_context as u64,
                checkpoint_total_bytes: pilot_checkpoint_total_bytes,
                optimizer_state_bytes: pilot_optimizer_state_bytes,
                activation_headroom_bytes: pilot_activation_headroom_bytes,
                expected_mean_tokens_per_second: pilot_mean_tokens_per_second,
                detail: String::from(
                    "The bounded pilot32m ablation keeps tokenizer replay and loss-accounting checks cheap enough to run before the actual lane while still preserving the same tokenizer, dataset identity, and stage program surface.",
                ),
            },
        ],
        bounded_ablation_configs: vec![
            PsionActualPretrainingBoundedAblationConfig {
                ablation_id: String::from("psion_actual_pretraining_internal128m_smoke"),
                ablation_family: String::from("actual_lane_bringup_family"),
                stage_config: artifact_ref(&root, &actual_stage_config_path)?,
                model_descriptor: artifact_ref(&root, &internal_descriptor_path)?,
                config_binding_id: String::from("psion_actual_pretraining_internal128m_anchor"),
                max_train_token_budget: 16_777_216,
                max_validation_token_budget: 524_288,
                max_optimizer_steps: 256,
                tokens_per_step: 65_536,
                consumption_target: String::from("actual_lane_smoke_and_bringup"),
                detail: String::from(
                    "The 128M smoke ablation reuses the actual-lane stage config with a short capped budget so launcher, accounting, and tokenizer bindings can be checked honestly before a longer admitted run.",
                ),
            },
            PsionActualPretrainingBoundedAblationConfig {
                ablation_id: String::from("psion_actual_pretraining_pilot32m_replay"),
                ablation_family: String::from("actual_lane_bringup_family"),
                stage_config: artifact_ref(&root, &pilot_stage_config_path)?,
                model_descriptor: artifact_ref(&root, &pilot_descriptor_path)?,
                config_binding_id: String::from(
                    "psion_actual_pretraining_pilot32m_replay_ablation",
                ),
                max_train_token_budget: pilot_train_token_budget,
                max_validation_token_budget: pilot_validation_token_budget,
                max_optimizer_steps: pilot_optimizer_steps,
                tokens_per_step: pilot_tokens_per_step,
                consumption_target: String::from("actual_lane_bounded_ablation_family"),
                detail: String::from(
                    "The pilot32m ablation is the bounded cheap replay lane that shares tokenizer and dataset truth with the actual lane while keeping model size and budget low enough for routine bring-up and accounting checks.",
                ),
            },
        ],
        support_refs: vec![
            String::from("docs/PSION_ACTUAL_PRETRAINING_BASELINE_TOOLS_BUNDLE.md"),
            String::from("docs/PSION_PRETRAIN_STAGE.md"),
            String::from("docs/PSION_TOKENIZER_TRAINING.md"),
            String::from("docs/TRAIN_SYSTEM.md"),
            String::from(
                "fixtures/psion/pretrain/psion_actual_pretraining_bringup_stage_config_v1.json",
            ),
            String::from(
                "fixtures/psion/pretrain/psion_actual_pretraining_pilot32m_ablation_stage_config_v1.json",
            ),
        ],
        claim_boundary: String::from(
            "The baseline-tools bundle ports selective CS336 A1 work into the actual lane by freezing one honest pretrain-stage bring-up surface, one tokenizer reproducibility binding, two resource-accounting rows, and two bounded ablation configs. It does not create a second pedagogical trainer stack or a detached curriculum lane.",
        ),
        summary: String::from(
            "The canonical actual-pretraining baseline-tools bundle binds bring-up trainer, tokenizer reproducibility, resource accounting, and bounded ablation configs directly into the actual lane.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = stable_baseline_tools_bundle_digest(&bundle)?;
    bundle.validate()?;
    write_json(
        &fixtures_dir.join("psion_actual_pretraining_baseline_tools_bundle_v1.json"),
        &bundle,
    )?;
    Ok(())
}

fn objective_config(
    descriptor: &PsionCompactDecoderDescriptor,
    tokenized_corpus_manifest: &PsionTokenizedCorpusManifest,
) -> PsionPretrainObjectiveConfig {
    PsionPretrainObjectiveConfig {
        objective_kind: PsionPretrainObjectiveKind::NextTokenPrediction,
        loss_normalization: PsionPretrainLossNormalization::ByTargetToken,
        label_smoothing_bps: 25,
        tokenizer_binding_digest: descriptor.tokenizer_binding.stable_digest(),
        dataset_identity: tokenized_corpus_manifest
            .replay_contract
            .stable_dataset_identity
            .clone(),
        max_context_tokens: descriptor.config.max_context,
    }
}

fn parameter_speedup_ratio(actual_parameter_count: u64, smaller_parameter_count: u64) -> f64 {
    (actual_parameter_count as f64 / smaller_parameter_count as f64).sqrt()
}

fn size_anchor_label(descriptor: &PsionCompactDecoderDescriptor) -> String {
    match descriptor.size_anchor {
        psionic_models::PsionCompactDecoderSizeAnchor::Pilot32m => String::from("pilot32m"),
        psionic_models::PsionCompactDecoderSizeAnchor::Internal128m => String::from("internal128m"),
    }
}

fn round_scaled(value: u64, ratio: f64) -> u64 {
    ((value as f64) * ratio).round() as u64
}

fn artifact_ref(
    root: &Path,
    path: &Path,
) -> Result<PsionActualPretrainingArtifactRef, Box<dyn Error>> {
    Ok(PsionActualPretrainingArtifactRef {
        path: path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/"),
        sha256: file_sha256(path)?,
    })
}

fn load_json<T>(path: &Path) -> Result<T, Box<dyn Error>>
where
    T: serde::de::DeserializeOwned,
{
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

fn write_json<T>(path: &Path, value: &T) -> Result<(), Box<dyn Error>>
where
    T: serde::Serialize,
{
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

fn file_sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    use sha2::{Digest, Sha256};

    let mut digest = Sha256::new();
    digest.update(fs::read(path)?);
    Ok(format!("{:x}", digest.finalize()))
}

fn u64_at(value: &serde_json::Value, path: &[&str]) -> Result<u64, Box<dyn Error>> {
    let mut current = value;
    for segment in path {
        current = if let Ok(index) = segment.parse::<usize>() {
            current.get(index)
        } else {
            current.get(*segment)
        }
        .ok_or_else(|| format!("missing field {}", path.join(".")))?;
    }
    current
        .as_u64()
        .ok_or_else(|| format!("field {} must be u64", path.join(".")).into())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
