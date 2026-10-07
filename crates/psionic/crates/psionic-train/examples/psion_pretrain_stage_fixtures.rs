use std::{error::Error, fs, path::PathBuf};

use psionic_data::{DatasetSplitKind, PsionTokenizedCorpusManifest};
use psionic_models::PsionCompactDecoderDescriptor;
use psionic_runtime::TrainingCheckpointReference;
use psionic_train::{
    run_psion_pretrain_stage, PsionPretrainCheckpointLineageReceipt,
    PsionPretrainLossNormalization, PsionPretrainObjectiveConfig, PsionPretrainObjectiveKind,
    PsionPretrainReplayReceipt, PsionPretrainSourceFamilyReportRow, PsionPretrainStageConfig,
    PsionSamplingPolicyManifest,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/psion/pretrain");
    fs::create_dir_all(&fixtures_dir)?;

    let model_descriptor: PsionCompactDecoderDescriptor =
        serde_json::from_str(&fs::read_to_string(root.join(
            "fixtures/psion/models/psion_compact_decoder_pilot_descriptor_v1.json",
        ))?)?;
    let tokenized_corpus: PsionTokenizedCorpusManifest =
        serde_json::from_str(&fs::read_to_string(
            root.join("fixtures/psion/tokenized/psion_tokenized_corpus_manifest_v1.json"),
        )?)?;
    let sampling_policy: PsionSamplingPolicyManifest = serde_json::from_str(&fs::read_to_string(
        root.join("fixtures/psion/sampling/psion_sampling_policy_manifest_v1.json"),
    )?)?;

    let stage_config = PsionPretrainStageConfig::new(
        "run-psion-pilot",
        "run-psion-pilot-stage-1-pretrain",
        PsionPretrainObjectiveConfig {
            objective_kind: PsionPretrainObjectiveKind::NextTokenPrediction,
            loss_normalization: PsionPretrainLossNormalization::ByTargetToken,
            label_smoothing_bps: 25,
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
        "psion-pretrain-replay-v1",
        tokenized_corpus.replay_contract.stable_dataset_identity.clone(),
        tokenized_corpus.replay_contract.iteration_mode,
        tokenized_corpus.replay_contract.shard_ordering,
        tokenized_corpus.replay_contract.deterministic_shuffle_seed,
        2,
        true,
        "Replay checks reproduced the tokenized-corpus order and deterministic shuffle contract on two independent dry runs.",
    );

    let promoted_checkpoint = TrainingCheckpointReference::new(
        "train.psion.decoder",
        "stream-psion-pretrain-final-v1",
        "manifest-psion-pretrain-final-v1",
        "object-psion-pretrain-final-v1",
        "node-psion-a",
        1,
        "cluster-state-digest-psion-pilot-v1",
        "topology-digest-psion-pilot-v1",
        1_742_614_800_000,
    )
    .with_checkpoint_ref("checkpoint://psion/pilot/pretrain/final")
    .with_step(2048)
    .with_durable_at_ms(1_742_615_100_000);

    let checkpoint_lineage = PsionPretrainCheckpointLineageReceipt::new(
        "psion-pretrain-checkpoint-lineage-v1",
        promoted_checkpoint,
        None,
        "pilot-pretrain-final",
        model_descriptor.model.model_id.clone(),
        model_descriptor.stable_digest(),
    );

    let source_family_reports = vec![
        PsionPretrainSourceFamilyReportRow {
            split_name: String::from("held_out"),
            split_kind: DatasetSplitKind::HeldOut,
            source_family_id: String::from("evaluation_only_benchmark_material"),
            source_ids: vec![String::from("spec_quiz_eval_pack_v1")],
            token_share_bps_within_split: 10_000,
            sequence_share_bps_within_split: 10_000,
            mean_next_token_loss_milli: 1380,
            detail: String::from(
                "Held-out benchmark material stays isolated and is only reported as held-out evidence.",
            ),
        },
        PsionPretrainSourceFamilyReportRow {
            split_name: String::from("train"),
            split_kind: DatasetSplitKind::Train,
            source_family_id: String::from("computer_architecture_history"),
            source_ids: vec![String::from("arch_textbook_foster_1985")],
            token_share_bps_within_split: 5600,
            sequence_share_bps_within_split: 5400,
            mean_next_token_loss_milli: 1125,
            detail: String::from(
                "Architecture-history prose remains the largest explanatory slice in train.",
            ),
        },
        PsionPretrainSourceFamilyReportRow {
            split_name: String::from("train"),
            split_kind: DatasetSplitKind::Train,
            source_family_id: String::from("normative_specs"),
            source_ids: vec![String::from("wasm_core_spec_release_2")],
            token_share_bps_within_split: 4400,
            sequence_share_bps_within_split: 4600,
            mean_next_token_loss_milli: 1190,
            detail: String::from(
                "Normative specification text stays large in train without overtaking the prose anchor.",
            ),
        },
        PsionPretrainSourceFamilyReportRow {
            split_name: String::from("validation"),
            split_kind: DatasetSplitKind::Validation,
            source_family_id: String::from("computer_architecture_history"),
            source_ids: vec![String::from("arch_textbook_foster_1985")],
            token_share_bps_within_split: 5300,
            sequence_share_bps_within_split: 5200,
            mean_next_token_loss_milli: 1160,
            detail: String::from(
                "Validation keeps the explanatory prose family slightly ahead to measure reasoning generalization.",
            ),
        },
        PsionPretrainSourceFamilyReportRow {
            split_name: String::from("validation"),
            split_kind: DatasetSplitKind::Validation,
            source_family_id: String::from("normative_specs"),
            source_ids: vec![String::from("wasm_core_spec_release_2")],
            token_share_bps_within_split: 4700,
            sequence_share_bps_within_split: 4800,
            mean_next_token_loss_milli: 1235,
            detail: String::from(
                "Validation specification coverage remains explicit so spec-reading drift is visible before promotion.",
            ),
        },
    ];

    let stage_receipt = run_psion_pretrain_stage(
        &stage_config,
        source_family_reports,
        replay_receipt,
        checkpoint_lineage,
        "Pilot Psion pretrain stage binds the compact decoder, replay-safe tokenized corpus, and sampling policy into one explicit next-token stage receipt.",
        &model_descriptor,
        &tokenized_corpus,
        &sampling_policy,
    )?;

    fs::write(
        fixtures_dir.join("psion_pretrain_stage_config_v1.json"),
        serde_json::to_string_pretty(&stage_config)?,
    )?;
    fs::write(
        fixtures_dir.join("psion_pretrain_stage_receipt_v1.json"),
        serde_json::to_string_pretty(&stage_receipt)?,
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
