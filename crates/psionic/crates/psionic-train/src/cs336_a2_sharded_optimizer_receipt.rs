use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use psionic_core::TensorData;
use psionic_nn::{
    ModuleStateDict, ModuleStateEntry, ModuleStateEntryKind, ModuleStateError, ModuleStateView,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    cs336_a1_get_lr_cosine_schedule, cs336_a1_gradient_clipping, Cs336A1ReferenceBatch,
    Cs336A1ReferenceTrainer, Cs336A1ReferenceTrainingConfig, Cs336A1ReferenceTrainingError,
    Cs336A1ReferenceTrainingStepReport, TrainingOptimizerConfig, TrainingOptimizerError,
    TrainingOptimizerShardResidency, TrainingOptimizerState, TrainingOptimizerStateShardKind,
    TrainingOptimizerStateShardLayout, TrainingParameterShardKind, TrainingParameterShardLayout,
    TrainingShardPlacement, TrainingShardRange, CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH,
    CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH, CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH,
    CS336_A2_REFERENCE_LANE_DOC_PATH,
};

pub const CS336_A2_SHARDED_OPTIMIZER_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_sharded_optimizer_receipt_v1.json";
pub const CS336_A2_SHARDED_OPTIMIZER_RECEIPT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.sharded_optimizer_receipt.v1";
const SHARDED_OPTIMIZER_PARITY_MAX_ABS_DELTA_TOLERANCE: f32 = 0.0005;

#[derive(Debug, Error)]
pub enum Cs336A2ShardedOptimizerReceiptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Training(#[from] Cs336A1ReferenceTrainingError),
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error(transparent)]
    Optimizer(#[from] TrainingOptimizerError),
    #[error("invalid CS336 A2 sharded optimizer receipt: {0}")]
    InvalidReceipt(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2ShardedOptimizerConfig {
    pub world_size: usize,
    pub training_steps: usize,
    pub a1_training: Cs336A1ReferenceTrainingConfig,
}

impl Default for Cs336A2ShardedOptimizerConfig {
    fn default() -> Self {
        let mut training = Cs336A1ReferenceTrainingConfig::tiny();
        training.batch_size = 2;
        training.max_learning_rate = 0.1;
        training.min_learning_rate = 0.1;
        training.warmup_iters = 0;
        training.cosine_cycle_iters = 8;
        training.adam_beta1 = 0.9;
        training.adam_beta2 = 0.999;
        training.adam_epsilon = 1e-8;
        training.weight_decay = 0.1;
        Self {
            world_size: 2,
            training_steps: 3,
            a1_training: training,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2ShardedOptimizerParameterOwnershipReceipt {
    pub parameter_path: String,
    pub parameter_element_count: usize,
    pub logical_optimizer_state_bytes: u64,
    pub owner_rank: usize,
    pub parameter_layout: TrainingParameterShardLayout,
    pub optimizer_state_layout: TrainingOptimizerStateShardLayout,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2ShardedOptimizerLayoutReceipt {
    pub world_size: usize,
    pub rank_device_labels: BTreeMap<usize, String>,
    pub parameter_shard_kind: TrainingParameterShardKind,
    pub optimizer_state_shard_kind: TrainingOptimizerStateShardKind,
    pub synchronization_posture: String,
    pub same_batch_on_all_ranks: bool,
    pub per_rank_owned_parameter_paths: BTreeMap<usize, Vec<String>>,
    pub per_rank_logical_optimizer_state_bytes: BTreeMap<usize, u64>,
    pub parameter_ownership: Vec<Cs336A2ShardedOptimizerParameterOwnershipReceipt>,
    pub ownership_is_disjoint_and_complete: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2ShardedOptimizerRankStepReceipt {
    pub rank: usize,
    pub owned_parameter_paths: Vec<String>,
    pub optimizer_state_entry_count_before: usize,
    pub optimizer_state_entry_count_after: usize,
    pub logical_optimizer_state_bytes_after: u64,
    pub optimizer_state_digest_before: String,
    pub optimizer_state_digest_after: String,
    pub updated_parameter_digests: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2ShardedOptimizerStepReceipt {
    pub step_number: u64,
    pub batch_start_positions: Vec<usize>,
    pub baseline_loss_before: f32,
    pub sharded_loss_before_by_rank: BTreeMap<usize, f32>,
    pub baseline_step: Cs336A1ReferenceTrainingStepReport,
    pub rank_steps: Vec<Cs336A2ShardedOptimizerRankStepReceipt>,
    pub combined_optimizer_state_digest_after: String,
    pub baseline_optimizer_state_digest_after: String,
    pub reconstructed_checkpoint_optimizer_state_matches_baseline: bool,
    pub rank0_baseline_max_model_delta: f32,
    pub rank1_rank0_max_model_delta: f32,
    pub combined_optimizer_baseline_max_delta: f32,
    pub rank0_matches_baseline: bool,
    pub rank1_matches_rank0: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2ShardedOptimizerReceipt {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub baseline_profile_bundle_path: String,
    pub bucketed_ddp_receipt_path: String,
    pub corpus_fixture_path: String,
    pub config: Cs336A2ShardedOptimizerConfig,
    pub layout: Cs336A2ShardedOptimizerLayoutReceipt,
    pub initial_model_state_digest: String,
    pub steps: Vec<Cs336A2ShardedOptimizerStepReceipt>,
    pub final_rank0_model_state_digest: String,
    pub final_rank1_model_state_digest: String,
    pub final_baseline_model_state_digest: String,
    pub final_optimizer_shard_digests_by_rank: BTreeMap<usize, String>,
    pub final_combined_optimizer_state_digest: String,
    pub final_baseline_optimizer_state_digest: String,
    pub all_steps_match_baseline: bool,
    pub claim_boundary: String,
}

pub fn build_cs336_a2_sharded_optimizer_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2ShardedOptimizerReceipt, Cs336A2ShardedOptimizerReceiptError> {
    let repo_root = repo_root.as_ref();
    let config = Cs336A2ShardedOptimizerConfig::default();
    if config.world_size != 2 {
        return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
            format!(
                "bounded sharded optimizer receipt currently expects world_size=2, got {}",
                config.world_size
            ),
        ));
    }

    let corpus_path = repo_root.join(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
    let mut baseline_trainer =
        Cs336A1ReferenceTrainer::from_corpus_path(&corpus_path, config.a1_training.clone())?;
    let mut rank0_trainer =
        Cs336A1ReferenceTrainer::from_corpus_path(&corpus_path, config.a1_training.clone())?;
    let mut rank1_trainer =
        Cs336A1ReferenceTrainer::from_corpus_path(&corpus_path, config.a1_training.clone())?;

    let initial_model_state_digest = rank0_trainer.model_state_digest();
    if initial_model_state_digest != baseline_trainer.model_state_digest()
        || initial_model_state_digest != rank1_trainer.model_state_digest()
    {
        return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
            "bounded sharded optimizer lane did not start from identical replicated model state"
                .into(),
        ));
    }

    let layout = build_layout_receipt(&rank0_trainer.model_state(), config.world_size)?;
    let parameter_paths = layout
        .parameter_ownership
        .iter()
        .map(|ownership| ownership.parameter_path.clone())
        .collect::<Vec<_>>();
    let mut optimizer_state_shards = initialize_empty_shards(config.world_size);
    let mut steps = Vec::with_capacity(config.training_steps);

    for step_index in 0..config.training_steps {
        let batch = baseline_trainer.batch_for_iteration(step_index as u64)?;
        let baseline_loss_before = baseline_trainer.loss_for_explicit_batch(&batch)?;
        let sharded_loss_before_by_rank = BTreeMap::from([
            (0usize, rank0_trainer.loss_for_explicit_batch(&batch)?),
            (1usize, rank1_trainer.loss_for_explicit_batch(&batch)?),
        ]);
        let raw_gradients = gradient_state_dict_for_paths(
            &rank0_trainer,
            &batch,
            *sharded_loss_before_by_rank.get(&0).ok_or_else(|| {
                Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
                    "missing rank 0 pre-step loss".into(),
                )
            })?,
            &parameter_paths,
        )?;
        let mut clipped_gradients = raw_gradients.clone();
        let _gradient_clip = cs336_a1_gradient_clipping(
            &mut clipped_gradients,
            config.a1_training.gradient_clip_norm,
        )?;
        let learning_rate = cs336_a1_get_lr_cosine_schedule(
            step_index as u64,
            config.a1_training.max_learning_rate,
            config.a1_training.min_learning_rate,
            config.a1_training.warmup_iters,
            config.a1_training.cosine_cycle_iters,
        );
        let optimizer_config = bounded_optimizer_config(&config.a1_training, learning_rate);
        let mut updated_state = rank0_trainer.model_state();
        let rank_steps = apply_sharded_optimizer_step(
            &layout,
            &optimizer_config,
            step_index as u64 + 1,
            &clipped_gradients,
            &mut updated_state,
            &mut optimizer_state_shards,
        )?;

        rank0_trainer.load_model_state(&updated_state)?;
        rank1_trainer.load_model_state(&updated_state)?;

        let baseline_step = baseline_trainer.apply_precomputed_gradients(
            &batch,
            baseline_loss_before,
            raw_gradients,
        )?;
        let combined_optimizer_state =
            combine_optimizer_state_shards(&layout, &optimizer_state_shards)?;
        let combined_optimizer_state_digest =
            optimizer_state_digest_for_map(&combined_optimizer_state);
        let baseline_optimizer_state_digest = baseline_trainer.optimizer_state_digest()?;
        let rank0_baseline_max_model_delta = module_state_dict_max_abs_delta(
            &rank0_trainer.model_state(),
            &baseline_trainer.model_state(),
        )?;
        let rank1_rank0_max_model_delta = module_state_dict_max_abs_delta(
            &rank1_trainer.model_state(),
            &rank0_trainer.model_state(),
        )?;
        let combined_optimizer_baseline_max_delta = optimizer_states_max_abs_delta(
            &combined_optimizer_state,
            baseline_trainer.optimizer_states(),
        )?;
        steps.push(Cs336A2ShardedOptimizerStepReceipt {
            step_number: baseline_step.step_number,
            batch_start_positions: batch.start_positions.clone(),
            baseline_loss_before,
            sharded_loss_before_by_rank,
            baseline_step,
            rank_steps,
            combined_optimizer_state_digest_after: combined_optimizer_state_digest.clone(),
            baseline_optimizer_state_digest_after: baseline_optimizer_state_digest.clone(),
            reconstructed_checkpoint_optimizer_state_matches_baseline:
                combined_optimizer_state_digest == baseline_optimizer_state_digest,
            rank0_baseline_max_model_delta,
            rank1_rank0_max_model_delta,
            combined_optimizer_baseline_max_delta,
            rank0_matches_baseline: rank0_baseline_max_model_delta
                <= SHARDED_OPTIMIZER_PARITY_MAX_ABS_DELTA_TOLERANCE
                && combined_optimizer_baseline_max_delta
                    <= SHARDED_OPTIMIZER_PARITY_MAX_ABS_DELTA_TOLERANCE,
            rank1_matches_rank0: rank1_rank0_max_model_delta
                <= SHARDED_OPTIMIZER_PARITY_MAX_ABS_DELTA_TOLERANCE,
        });
    }

    let final_combined_optimizer_state =
        combine_optimizer_state_shards(&layout, &optimizer_state_shards)?;
    let final_optimizer_shard_digests_by_rank = optimizer_state_shards
        .iter()
        .map(|(rank, shard)| (*rank, optimizer_state_digest_for_map(shard)))
        .collect::<BTreeMap<_, _>>();

    let receipt = finalize_receipt(Cs336A2ShardedOptimizerReceipt {
        schema_version: String::from(CS336_A2_SHARDED_OPTIMIZER_RECEIPT_SCHEMA_VERSION),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
        baseline_profile_bundle_path: String::from(CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH),
        bucketed_ddp_receipt_path: String::from(CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH),
        corpus_fixture_path: String::from(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH),
        config,
        layout,
        initial_model_state_digest,
        steps,
        final_rank0_model_state_digest: rank0_trainer.model_state_digest(),
        final_rank1_model_state_digest: rank1_trainer.model_state_digest(),
        final_baseline_model_state_digest: baseline_trainer.model_state_digest(),
        final_optimizer_shard_digests_by_rank,
        final_combined_optimizer_state_digest: optimizer_state_digest_for_map(
            &final_combined_optimizer_state,
        ),
        final_baseline_optimizer_state_digest: baseline_trainer.optimizer_state_digest()?,
        all_steps_match_baseline: false,
        claim_boundary: String::from(
            "This receipt proves a bounded CS336 A2 sharded-optimizer lane inside psionic. It keeps model parameters replicated across two local reference ranks, assigns AdamW optimizer state ownership by parameter path, applies owner-only updates against the clipped global finite-difference gradient surface from the owned A1 trainer, and then rebroadcasts the updated parameters so both ranks converge back to the same model state. The retained combined optimizer-state digest matches the non-sharded baseline after each bounded step. It does not claim actual transport-backed ZeRO execution, real checkpoint partition exchange, or actual-lane distributed qualification.",
        ),
    });
    validate_receipt(&receipt)?;
    Ok(receipt)
}

pub fn write_cs336_a2_sharded_optimizer_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2ShardedOptimizerReceipt, Cs336A2ShardedOptimizerReceiptError> {
    let receipt = build_cs336_a2_sharded_optimizer_receipt(&repo_root)?;
    let receipt_path = repo_root
        .as_ref()
        .join(CS336_A2_SHARDED_OPTIMIZER_RECEIPT_FIXTURE_PATH);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

fn initialize_empty_shards(
    world_size: usize,
) -> BTreeMap<usize, BTreeMap<String, TrainingOptimizerState>> {
    (0..world_size)
        .map(|rank| (rank, BTreeMap::new()))
        .collect::<BTreeMap<_, _>>()
}

fn bounded_optimizer_config(
    config: &Cs336A1ReferenceTrainingConfig,
    learning_rate: f32,
) -> TrainingOptimizerConfig {
    TrainingOptimizerConfig::adamw(
        learning_rate,
        config.adam_beta1,
        config.adam_beta2,
        config.adam_epsilon,
    )
    .with_weight_decay(config.weight_decay)
}

fn build_layout_receipt(
    model_state: &ModuleStateDict,
    world_size: usize,
) -> Result<Cs336A2ShardedOptimizerLayoutReceipt, Cs336A2ShardedOptimizerReceiptError> {
    if world_size == 0 {
        return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
            "world_size must be positive".into(),
        ));
    }

    let rank_device_labels = (0..world_size)
        .map(|rank| (rank, format!("cpu:{rank}")))
        .collect::<BTreeMap<_, _>>();
    let mut per_rank_owned_parameter_paths = (0..world_size)
        .map(|rank| (rank, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    let mut per_rank_logical_optimizer_state_bytes = (0..world_size)
        .map(|rank| (rank, 0_u64))
        .collect::<BTreeMap<_, _>>();
    let mut parameter_ownership = Vec::new();

    for (index, (path, entry)) in model_state
        .entries
        .iter()
        .filter(|(_, entry)| entry.kind == ModuleStateEntryKind::Parameter && entry.requires_grad)
        .enumerate()
    {
        let owner_rank = index % world_size;
        let values = dense_tensor_values(entry)?;
        let logical_optimizer_state_bytes = (values.len() * 2 * std::mem::size_of::<f32>()) as u64;
        per_rank_owned_parameter_paths
            .get_mut(&owner_rank)
            .ok_or_else(|| {
                Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                    "missing owned-path bucket for rank {owner_rank}"
                ))
            })?
            .push(path.clone());
        *per_rank_logical_optimizer_state_bytes
            .get_mut(&owner_rank)
            .ok_or_else(|| {
                Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                    "missing logical-byte bucket for rank {owner_rank}"
                ))
            })? += logical_optimizer_state_bytes;

        parameter_ownership.push(Cs336A2ShardedOptimizerParameterOwnershipReceipt {
            parameter_path: path.clone(),
            parameter_element_count: values.len(),
            logical_optimizer_state_bytes,
            owner_rank,
            parameter_layout: TrainingParameterShardLayout::new(
                TrainingParameterShardKind::Replicated,
                (0..world_size)
                    .map(|rank| {
                        TrainingShardPlacement::new(
                            rank,
                            "dp",
                            format!("rank-{rank}"),
                            format!("cpu:{rank}"),
                            rank,
                            TrainingShardRange::new(0, values.len()),
                        )
                    })
                    .collect(),
            )
            .with_axis_id("dp"),
            optimizer_state_layout: TrainingOptimizerStateShardLayout::new(
                TrainingOptimizerStateShardKind::ZeroStage1,
                TrainingOptimizerShardResidency::HostOffloaded,
                vec![TrainingShardPlacement::new(
                    owner_rank,
                    "dp",
                    format!("rank-{owner_rank}"),
                    format!("cpu:{owner_rank}"),
                    owner_rank,
                    TrainingShardRange::new(0, values.len() * 2),
                )],
            )
            .with_axis_id("dp"),
        });
    }

    if parameter_ownership.is_empty() {
        return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
            "missing trainable parameters for bounded sharded optimizer layout".into(),
        ));
    }

    let mut seen = BTreeSet::new();
    let mut duplicate_seen = false;
    for owned_paths in per_rank_owned_parameter_paths.values() {
        for path in owned_paths {
            if !seen.insert(path.clone()) {
                duplicate_seen = true;
            }
        }
    }
    let ownership_is_disjoint_and_complete =
        !duplicate_seen && seen.len() == parameter_ownership.len();

    Ok(Cs336A2ShardedOptimizerLayoutReceipt {
        world_size,
        rank_device_labels,
        parameter_shard_kind: TrainingParameterShardKind::Replicated,
        optimizer_state_shard_kind: TrainingOptimizerStateShardKind::ZeroStage1,
        synchronization_posture: String::from(
            "same_batch_on_all_ranks_then_owner_step_then_parameter_rebroadcast",
        ),
        same_batch_on_all_ranks: true,
        per_rank_owned_parameter_paths,
        per_rank_logical_optimizer_state_bytes,
        parameter_ownership,
        ownership_is_disjoint_and_complete,
    })
}

fn apply_sharded_optimizer_step(
    layout: &Cs336A2ShardedOptimizerLayoutReceipt,
    optimizer_config: &TrainingOptimizerConfig,
    step_number: u64,
    clipped_gradients: &ModuleStateDict,
    updated_model_state: &mut ModuleStateDict,
    optimizer_state_shards: &mut BTreeMap<usize, BTreeMap<String, TrainingOptimizerState>>,
) -> Result<Vec<Cs336A2ShardedOptimizerRankStepReceipt>, Cs336A2ShardedOptimizerReceiptError> {
    let mut rank_steps = Vec::with_capacity(layout.world_size);
    for rank in 0..layout.world_size {
        let owned_parameter_paths = layout
            .per_rank_owned_parameter_paths
            .get(&rank)
            .cloned()
            .ok_or_else(|| {
                Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                    "missing owned parameter list for rank {rank}"
                ))
            })?;
        let shard = optimizer_state_shards.get_mut(&rank).ok_or_else(|| {
            Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                "missing optimizer shard for rank {rank}"
            ))
        })?;
        let optimizer_state_digest_before = optimizer_state_digest_for_map(shard);
        let optimizer_state_entry_count_before = shard.len();
        let mut updated_parameter_digests = BTreeMap::new();
        for path in &owned_parameter_paths {
            let gradient_entry = clipped_gradients.entries.get(path).ok_or_else(|| {
                Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                    "missing clipped gradient entry `{path}` during sharded optimizer step"
                ))
            })?;
            let parameter_entry = updated_model_state.entries.get_mut(path).ok_or_else(|| {
                Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                    "missing model parameter entry `{path}` during sharded optimizer step"
                ))
            })?;
            let parameter_values = dense_tensor_values_mut(parameter_entry)?;
            let gradient_values = dense_tensor_values(gradient_entry)?;
            let optimizer_state = shard
                .entry(path.clone())
                .or_insert_with(|| optimizer_config.initialize_state(parameter_values.len()));
            optimizer_config.apply_step(
                parameter_values,
                gradient_values,
                optimizer_state,
                step_number,
            )?;
            updated_parameter_digests.insert(
                path.clone(),
                stable_json_digest(
                    b"psion.cs336_a2.sharded_optimizer.updated_parameter",
                    &parameter_values,
                ),
            );
        }
        rank_steps.push(Cs336A2ShardedOptimizerRankStepReceipt {
            rank,
            owned_parameter_paths,
            optimizer_state_entry_count_before,
            optimizer_state_entry_count_after: shard.len(),
            logical_optimizer_state_bytes_after: logical_optimizer_state_bytes_for_map(shard),
            optimizer_state_digest_before,
            optimizer_state_digest_after: optimizer_state_digest_for_map(shard),
            updated_parameter_digests,
        });
    }
    Ok(rank_steps)
}

fn combine_optimizer_state_shards(
    layout: &Cs336A2ShardedOptimizerLayoutReceipt,
    optimizer_state_shards: &BTreeMap<usize, BTreeMap<String, TrainingOptimizerState>>,
) -> Result<BTreeMap<String, TrainingOptimizerState>, Cs336A2ShardedOptimizerReceiptError> {
    let mut combined = BTreeMap::new();
    for ownership in &layout.parameter_ownership {
        let owner_shard = optimizer_state_shards
            .get(&ownership.owner_rank)
            .ok_or_else(|| {
                Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                    "missing owner shard for rank {}",
                    ownership.owner_rank
                ))
            })?;
        let state = owner_shard
            .get(&ownership.parameter_path)
            .cloned()
            .ok_or_else(|| {
                Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                    "missing optimizer state `{}` on owner rank {}",
                    ownership.parameter_path, ownership.owner_rank
                ))
            })?;
        if combined
            .insert(ownership.parameter_path.clone(), state)
            .is_some()
        {
            return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
                "duplicate optimizer-state ownership while combining bounded shards".into(),
            ));
        }
    }
    Ok(combined)
}

fn finalize_receipt(mut receipt: Cs336A2ShardedOptimizerReceipt) -> Cs336A2ShardedOptimizerReceipt {
    receipt.all_steps_match_baseline = receipt.layout.ownership_is_disjoint_and_complete
        && receipt.steps.iter().all(|step| {
            step.rank0_matches_baseline
                && step.rank1_matches_rank0
                && step.reconstructed_checkpoint_optimizer_state_matches_baseline
        })
        && receipt.final_rank0_model_state_digest == receipt.final_baseline_model_state_digest
        && receipt.final_rank1_model_state_digest == receipt.final_rank0_model_state_digest
        && receipt.final_combined_optimizer_state_digest
            == receipt.final_baseline_optimizer_state_digest;
    receipt
}

fn validate_receipt(
    receipt: &Cs336A2ShardedOptimizerReceipt,
) -> Result<(), Cs336A2ShardedOptimizerReceiptError> {
    if receipt.schema_version != CS336_A2_SHARDED_OPTIMIZER_RECEIPT_SCHEMA_VERSION {
        return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
            "expected schema version `{CS336_A2_SHARDED_OPTIMIZER_RECEIPT_SCHEMA_VERSION}`, got `{}`",
            receipt.schema_version
        )));
    }
    if !receipt.layout.ownership_is_disjoint_and_complete {
        return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
            "sharded optimizer ownership is not disjoint and complete".into(),
        ));
    }
    if !receipt.all_steps_match_baseline {
        return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
            "bounded sharded optimizer steps did not stay aligned with the non-sharded baseline"
                .into(),
        ));
    }
    Ok(())
}

fn gradient_state_dict_for_paths(
    trainer: &Cs336A1ReferenceTrainer,
    batch: &Cs336A1ReferenceBatch,
    base_loss: f32,
    parameter_paths: &[String],
) -> Result<ModuleStateDict, Cs336A2ShardedOptimizerReceiptError> {
    let model_state = trainer.model_state();
    let mut entries = BTreeMap::new();
    for path in parameter_paths {
        entries.insert(
            path.clone(),
            trainer.estimate_parameter_gradient(batch, base_loss, path.as_str())?,
        );
    }
    Ok(ModuleStateDict::new(
        model_state.root_module_id.clone(),
        model_state.root_module_kind.clone(),
        ModuleStateView::PersistentOnly,
        entries,
    )?)
}

fn dense_tensor_values(
    entry: &ModuleStateEntry,
) -> Result<&[f32], Cs336A2ShardedOptimizerReceiptError> {
    match &entry.data {
        TensorData::F32(values) => Ok(values.as_slice()),
        other => Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
            format!(
                "expected dense f32 parameter entry for `{}`, found {other:?}",
                entry.path
            ),
        )),
    }
}

fn dense_tensor_values_mut(
    entry: &mut ModuleStateEntry,
) -> Result<&mut [f32], Cs336A2ShardedOptimizerReceiptError> {
    match &mut entry.data {
        TensorData::F32(values) => Ok(values.as_mut_slice()),
        other => Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(
            format!(
                "expected mutable dense f32 parameter entry for `{}`, found {other:?}",
                entry.path
            ),
        )),
    }
}

fn stable_json_digest<T: Serialize>(domain: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(serde_json::to_vec(value).expect("serializing stable digest input must succeed"));
    format!("{:x}", digest.finalize())
}

fn optimizer_state_digest_for_map(
    optimizer_states: &BTreeMap<String, TrainingOptimizerState>,
) -> String {
    stable_json_digest(
        b"psion.cs336_a1.reference_optimizer_state",
        optimizer_states,
    )
}

fn logical_optimizer_state_bytes_for_map(
    optimizer_states: &BTreeMap<String, TrainingOptimizerState>,
) -> u64 {
    optimizer_states
        .values()
        .map(|state| optimizer_state_values(state).len() as u64 * std::mem::size_of::<f32>() as u64)
        .sum()
}

fn module_state_dict_max_abs_delta(
    left: &ModuleStateDict,
    right: &ModuleStateDict,
) -> Result<f32, Cs336A2ShardedOptimizerReceiptError> {
    let mut max_delta = 0.0_f32;
    for (path, left_entry) in &left.entries {
        let right_entry = right.entries.get(path).ok_or_else(|| {
            Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                "missing state entry `{path}` while computing sharded optimizer parity delta"
            ))
        })?;
        let left_values = dense_tensor_values(left_entry)?;
        let right_values = dense_tensor_values(right_entry)?;
        if left_values.len() != right_values.len() {
            return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                "state entry `{path}` length mismatch while computing sharded optimizer parity delta"
            )));
        }
        for (left_value, right_value) in left_values.iter().zip(right_values.iter()) {
            max_delta = max_delta.max((left_value - right_value).abs());
        }
    }
    Ok(max_delta)
}

fn optimizer_states_max_abs_delta(
    left: &BTreeMap<String, TrainingOptimizerState>,
    right: &BTreeMap<String, TrainingOptimizerState>,
) -> Result<f32, Cs336A2ShardedOptimizerReceiptError> {
    let mut max_delta = 0.0_f32;
    for (path, left_state) in left {
        let right_state = right.get(path).ok_or_else(|| {
            Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                "missing optimizer state `{path}` while computing sharded optimizer parity delta"
            ))
        })?;
        let left_values = optimizer_state_values(left_state);
        let right_values = optimizer_state_values(right_state);
        if left_values.len() != right_values.len() {
            return Err(Cs336A2ShardedOptimizerReceiptError::InvalidReceipt(format!(
                "optimizer state `{path}` length mismatch while computing sharded optimizer parity delta"
            )));
        }
        for (left_value, right_value) in left_values.iter().zip(right_values.iter()) {
            max_delta = max_delta.max((left_value - right_value).abs());
        }
    }
    Ok(max_delta)
}

fn optimizer_state_values(state: &TrainingOptimizerState) -> Vec<f32> {
    match state {
        TrainingOptimizerState::Sgd { momentum_buffer }
        | TrainingOptimizerState::Lars { momentum_buffer } => {
            momentum_buffer.clone().unwrap_or_default()
        }
        TrainingOptimizerState::Adam {
            first_moment,
            second_moment,
        }
        | TrainingOptimizerState::AdamW {
            first_moment,
            second_moment,
        }
        | TrainingOptimizerState::Lamb {
            first_moment,
            second_moment,
        } => {
            let mut values = Vec::with_capacity(first_moment.len() + second_moment.len());
            values.extend_from_slice(first_moment);
            values.extend_from_slice(second_moment);
            values
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tempfile::tempdir;

    use super::{
        build_cs336_a2_sharded_optimizer_receipt, write_cs336_a2_sharded_optimizer_receipt,
        CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH,
        CS336_A2_SHARDED_OPTIMIZER_RECEIPT_FIXTURE_PATH,
    };

    #[test]
    fn sharded_optimizer_receipt_matches_non_sharded_baseline(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let receipt = build_cs336_a2_sharded_optimizer_receipt(repo_root)?;
        assert!(receipt.layout.ownership_is_disjoint_and_complete);
        assert!(receipt.all_steps_match_baseline);
        assert_eq!(
            receipt.final_combined_optimizer_state_digest,
            receipt.final_baseline_optimizer_state_digest
        );
        Ok(())
    }

    #[test]
    fn sharded_optimizer_layout_is_partitioned() -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let receipt = build_cs336_a2_sharded_optimizer_receipt(repo_root)?;
        let rank0_paths = receipt
            .layout
            .per_rank_owned_parameter_paths
            .get(&0)
            .ok_or("missing rank 0 owned paths")?;
        let rank1_paths = receipt
            .layout
            .per_rank_owned_parameter_paths
            .get(&1)
            .ok_or("missing rank 1 owned paths")?;
        assert!(!rank0_paths.is_empty());
        assert!(!rank1_paths.is_empty());
        let disjoint = rank0_paths
            .iter()
            .all(|path| !rank1_paths.iter().any(|other| other == path));
        assert!(disjoint);
        assert_eq!(
            rank0_paths.len() + rank1_paths.len(),
            receipt.layout.parameter_ownership.len()
        );
        Ok(())
    }

    #[test]
    fn sharded_optimizer_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let corpus_src = repo_root.join(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
        let corpus_dst = temp
            .path()
            .join(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
        if let Some(parent) = corpus_dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(corpus_src, &corpus_dst)?;
        let receipt = write_cs336_a2_sharded_optimizer_receipt(temp.path())?;
        let fixture_path = temp
            .path()
            .join(CS336_A2_SHARDED_OPTIMIZER_RECEIPT_FIXTURE_PATH);
        assert!(fixture_path.exists());
        let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&fixture_path)?)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(receipt.schema_version.as_str())
        );
        Ok(())
    }
}
