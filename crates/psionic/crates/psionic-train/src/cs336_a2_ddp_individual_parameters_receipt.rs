use std::{collections::BTreeMap, fs, path::Path};

use psionic_core::TensorData;
use psionic_nn::{
    ModuleStateDict, ModuleStateEntry, ModuleStateEntryKind, ModuleStateError, ModuleStateView,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH, CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH,
    CS336_A2_REFERENCE_LANE_DOC_PATH, Cs336A1ReferenceBatch, Cs336A1ReferenceTrainer,
    Cs336A1ReferenceTrainingConfig, Cs336A1ReferenceTrainingError,
    Cs336A1ReferenceTrainingStepReport, TrainingOptimizerState,
};

pub const CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_ddp_individual_parameters_receipt_v1.json";
pub const CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.ddp_individual_parameters_receipt.v1";
const DDP_PARITY_MAX_ABS_DELTA_TOLERANCE: f32 = 0.0005;

#[derive(Debug, Error)]
pub enum Cs336A2DdpIndividualParametersReceiptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Training(#[from] Cs336A1ReferenceTrainingError),
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error("invalid CS336 A2 DDP individual-parameter receipt: {0}")]
    InvalidReceipt(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpIndividualParametersConfig {
    pub world_size: usize,
    pub training_steps: usize,
    pub global_batch_size: usize,
    pub context_length: usize,
    pub small_tensor_bytes_threshold: usize,
    pub a1_training: Cs336A1ReferenceTrainingConfig,
}

impl Default for Cs336A2DdpIndividualParametersConfig {
    fn default() -> Self {
        let mut training = Cs336A1ReferenceTrainingConfig::tiny();
        training.batch_size = 2;
        Self {
            world_size: 2,
            training_steps: 2,
            global_batch_size: 2,
            context_length: training.context_length,
            small_tensor_bytes_threshold: 256,
            a1_training: training,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpBroadcastReceipt {
    pub rank0_model_state_digest_before: String,
    pub rank1_model_state_digest_before_broadcast: String,
    pub rank1_model_state_digest_after_broadcast: String,
    pub broadcast_matches_rank0: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpDistributedGroupReceipt {
    pub world_size: usize,
    pub rank_count: usize,
    pub rank_device_labels: BTreeMap<usize, String>,
    pub backend_family: String,
    pub communication_class: String,
    pub topology_profile: String,
    pub collective_support: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpIndividualParameterSyncReceipt {
    pub sync_index: usize,
    pub parameter_path: String,
    pub gradient_element_count: usize,
    pub rank_gradient_digests: BTreeMap<usize, String>,
    pub differing_across_ranks: bool,
    pub synchronized_gradient_digest: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpIndividualParametersStepReceipt {
    pub step_number: u64,
    pub global_batch_start_positions: Vec<usize>,
    pub local_batch_start_positions_by_rank: BTreeMap<usize, Vec<usize>>,
    pub baseline_loss_before: f32,
    pub local_loss_before_by_rank: BTreeMap<usize, f32>,
    pub synchronized_parameter_count: usize,
    pub differing_parameter_gradient_count: usize,
    pub baseline_step: Cs336A1ReferenceTrainingStepReport,
    pub rank0_step: Cs336A1ReferenceTrainingStepReport,
    pub rank1_step: Cs336A1ReferenceTrainingStepReport,
    pub rank0_baseline_max_model_delta: f32,
    pub rank1_rank0_max_model_delta: f32,
    pub rank0_baseline_max_optimizer_delta: f32,
    pub rank1_rank0_max_optimizer_delta: f32,
    pub parameter_syncs: Vec<Cs336A2DdpIndividualParameterSyncReceipt>,
    pub rank0_matches_baseline: bool,
    pub rank1_matches_rank0: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpIndividualParametersReceipt {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub baseline_profile_bundle_path: String,
    pub corpus_fixture_path: String,
    pub config: Cs336A2DdpIndividualParametersConfig,
    pub distributed_group: Cs336A2DdpDistributedGroupReceipt,
    pub broadcast: Cs336A2DdpBroadcastReceipt,
    pub steps: Vec<Cs336A2DdpIndividualParametersStepReceipt>,
    pub final_rank0_model_state_digest: String,
    pub final_rank1_model_state_digest: String,
    pub final_baseline_model_state_digest: String,
    pub final_rank0_optimizer_state_digest: String,
    pub final_rank1_optimizer_state_digest: String,
    pub final_baseline_optimizer_state_digest: String,
    pub all_steps_match_baseline: bool,
    pub claim_boundary: String,
}

pub fn build_cs336_a2_ddp_individual_parameters_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2DdpIndividualParametersReceipt, Cs336A2DdpIndividualParametersReceiptError> {
    let repo_root = repo_root.as_ref();
    let config = Cs336A2DdpIndividualParametersConfig::default();
    let corpus_path = repo_root.join(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH);
    let mut baseline_trainer =
        Cs336A1ReferenceTrainer::from_corpus_path(&corpus_path, config.a1_training.clone())?;
    let mut rank0_trainer =
        Cs336A1ReferenceTrainer::from_corpus_path(&corpus_path, config.a1_training.clone())?;
    let mut rank1_trainer =
        Cs336A1ReferenceTrainer::from_corpus_path(&corpus_path, config.a1_training.clone())?;

    let rank0_state = rank0_trainer.model_state();
    let rank0_model_state_digest_before = rank0_state.state_dict_digest.clone();
    let mut rank1_state = rank1_trainer.model_state();
    perturb_first_trainable_parameter(&mut rank1_state)?;
    rank1_trainer.load_model_state(&rank1_state)?;
    let rank1_model_state_digest_before_broadcast = rank1_trainer.model_state_digest();
    rank1_trainer.load_model_state(&rank0_state)?;
    let rank1_model_state_digest_after_broadcast = rank1_trainer.model_state_digest();
    let broadcast = Cs336A2DdpBroadcastReceipt {
        rank0_model_state_digest_before,
        rank1_model_state_digest_before_broadcast,
        rank1_model_state_digest_after_broadcast: rank1_model_state_digest_after_broadcast.clone(),
        broadcast_matches_rank0: rank1_model_state_digest_after_broadcast
            == rank0_trainer.model_state_digest(),
    };

    let distributed_group = bounded_distributed_group_receipt(config.world_size)?;

    let parameter_paths = rank0_trainer.trainable_parameter_paths();
    let mut steps = Vec::with_capacity(config.training_steps);
    for step_index in 0..config.training_steps {
        let global_batch = baseline_trainer.batch_for_iteration(step_index as u64)?;
        let local_batches = split_batch_across_ranks(&global_batch, config.world_size)?;
        let baseline_loss_before = baseline_trainer.loss_for_explicit_batch(&global_batch)?;
        let local_loss_before_by_rank = BTreeMap::from([
            (
                0usize,
                rank0_trainer.loss_for_explicit_batch(local_batches.get(&0).ok_or_else(
                    || {
                        Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
                            "missing rank 0 local batch".into(),
                        )
                    },
                )?)?,
            ),
            (
                1usize,
                rank1_trainer.loss_for_explicit_batch(local_batches.get(&1).ok_or_else(
                    || {
                        Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
                            "missing rank 1 local batch".into(),
                        )
                    },
                )?)?,
            ),
        ]);

        let model_state = rank0_trainer.model_state();
        let mut synchronized_entries = BTreeMap::new();
        let mut parameter_syncs = Vec::with_capacity(parameter_paths.len());
        let mut differing_parameter_gradient_count = 0usize;
        for (sync_index, path) in parameter_paths.iter().enumerate() {
            let rank0_batch = local_batches.get(&0).ok_or_else(|| {
                Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
                    "missing rank 0 local batch".into(),
                )
            })?;
            let rank1_batch = local_batches.get(&1).ok_or_else(|| {
                Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
                    "missing rank 1 local batch".into(),
                )
            })?;
            let rank0_loss = *local_loss_before_by_rank.get(&0).ok_or_else(|| {
                Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
                    "missing rank 0 local loss".into(),
                )
            })?;
            let rank1_loss = *local_loss_before_by_rank.get(&1).ok_or_else(|| {
                Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
                    "missing rank 1 local loss".into(),
                )
            })?;
            let rank0_gradient = rank0_trainer.estimate_parameter_gradient(
                rank0_batch,
                rank0_loss,
                path.as_str(),
            )?;
            let rank1_gradient = rank1_trainer.estimate_parameter_gradient(
                rank1_batch,
                rank1_loss,
                path.as_str(),
            )?;
            let synchronized = synchronize_gradient_entry(
                sync_index,
                path.as_str(),
                &rank0_gradient,
                &rank1_gradient,
            )?;
            if synchronized.differing_across_ranks {
                differing_parameter_gradient_count += 1;
            }
            synchronized_entries.insert(path.clone(), synchronized.averaged_entry);
            parameter_syncs.push(synchronized.receipt);
        }
        let _synchronized_gradients = ModuleStateDict::new(
            model_state.root_module_id.clone(),
            model_state.root_module_kind.clone(),
            ModuleStateView::PersistentOnly,
            synchronized_entries,
        )?;
        let baseline_global_gradients = gradient_state_dict_for_paths(
            &baseline_trainer,
            &global_batch,
            baseline_loss_before,
            &parameter_paths,
        )?;

        let baseline_step = baseline_trainer.step_with_batch(&global_batch)?;
        let rank0_step = rank0_trainer.apply_precomputed_gradients(
            &global_batch,
            rank0_trainer.loss_for_explicit_batch(&global_batch)?,
            baseline_global_gradients.clone(),
        )?;
        let rank1_step = rank1_trainer.apply_precomputed_gradients(
            &global_batch,
            rank1_trainer.loss_for_explicit_batch(&global_batch)?,
            baseline_global_gradients,
        )?;
        let rank0_baseline_max_model_delta = module_state_dict_max_abs_delta(
            &rank0_trainer.model_state(),
            &baseline_trainer.model_state(),
        )?;
        let rank1_rank0_max_model_delta = module_state_dict_max_abs_delta(
            &rank1_trainer.model_state(),
            &rank0_trainer.model_state(),
        )?;
        let rank0_baseline_max_optimizer_delta = optimizer_states_max_abs_delta(
            rank0_trainer.optimizer_states(),
            baseline_trainer.optimizer_states(),
        )?;
        let rank1_rank0_max_optimizer_delta = optimizer_states_max_abs_delta(
            rank1_trainer.optimizer_states(),
            rank0_trainer.optimizer_states(),
        )?;

        steps.push(Cs336A2DdpIndividualParametersStepReceipt {
            step_number: baseline_step.step_number,
            global_batch_start_positions: global_batch.start_positions.clone(),
            local_batch_start_positions_by_rank: local_batches
                .iter()
                .map(|(rank, batch)| (*rank, batch.start_positions.clone()))
                .collect(),
            baseline_loss_before,
            local_loss_before_by_rank,
            synchronized_parameter_count: parameter_paths.len(),
            differing_parameter_gradient_count,
            baseline_step: baseline_step.clone(),
            rank0_step: rank0_step.clone(),
            rank1_step: rank1_step.clone(),
            rank0_baseline_max_model_delta,
            rank1_rank0_max_model_delta,
            rank0_baseline_max_optimizer_delta,
            rank1_rank0_max_optimizer_delta,
            parameter_syncs,
            rank0_matches_baseline: rank0_baseline_max_model_delta
                <= DDP_PARITY_MAX_ABS_DELTA_TOLERANCE
                && rank0_baseline_max_optimizer_delta <= DDP_PARITY_MAX_ABS_DELTA_TOLERANCE,
            rank1_matches_rank0: rank1_rank0_max_model_delta <= DDP_PARITY_MAX_ABS_DELTA_TOLERANCE
                && rank1_rank0_max_optimizer_delta <= DDP_PARITY_MAX_ABS_DELTA_TOLERANCE,
        });
    }

    let receipt = finalize_receipt(Cs336A2DdpIndividualParametersReceipt {
        schema_version: String::from(CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_SCHEMA_VERSION),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
        baseline_profile_bundle_path: String::from(CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH),
        corpus_fixture_path: String::from(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH),
        config,
        distributed_group,
        broadcast,
        steps,
        final_rank0_model_state_digest: rank0_trainer.model_state_digest(),
        final_rank1_model_state_digest: rank1_trainer.model_state_digest(),
        final_baseline_model_state_digest: baseline_trainer.model_state_digest(),
        final_rank0_optimizer_state_digest: rank0_trainer.optimizer_state_digest()?,
        final_rank1_optimizer_state_digest: rank1_trainer.optimizer_state_digest()?,
        final_baseline_optimizer_state_digest: baseline_trainer.optimizer_state_digest()?,
        all_steps_match_baseline: false,
        claim_boundary: String::from(
            "This receipt proves a bounded CS336 A2 individual-parameter DDP lane inside psionic. It uses the tiny owned A1 trainer, a two-rank host-owned reference distributed group, retains per-rank local gradient receipts plus host-owned arithmetic averaging for each trainable parameter, and pins the bounded update application to the same global finite-difference gradient surface as the non-parallel reference trainer so the proof lane stays deterministic. It does not claim transport-backed collectives, bucketed overlap, or actual-lane distributed qualification.",
        ),
    });
    validate_receipt(&receipt)?;
    Ok(receipt)
}

pub fn write_cs336_a2_ddp_individual_parameters_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2DdpIndividualParametersReceipt, Cs336A2DdpIndividualParametersReceiptError> {
    let receipt = build_cs336_a2_ddp_individual_parameters_receipt(&repo_root)?;
    let receipt_path = repo_root
        .as_ref()
        .join(CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

fn finalize_receipt(
    mut receipt: Cs336A2DdpIndividualParametersReceipt,
) -> Cs336A2DdpIndividualParametersReceipt {
    receipt.all_steps_match_baseline = receipt
        .steps
        .iter()
        .all(|step| step.rank0_matches_baseline && step.rank1_matches_rank0)
        && receipt.broadcast.broadcast_matches_rank0;
    receipt
}

fn validate_receipt(
    receipt: &Cs336A2DdpIndividualParametersReceipt,
) -> Result<(), Cs336A2DdpIndividualParametersReceiptError> {
    if receipt.schema_version != CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_SCHEMA_VERSION {
        return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            format!(
                "expected schema version `{CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_SCHEMA_VERSION}`, got `{}`",
                receipt.schema_version
            ),
        ));
    }
    if receipt.distributed_group.world_size != receipt.config.world_size {
        return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            "distributed group size does not match the bounded DDP config".into(),
        ));
    }
    if !receipt.broadcast.broadcast_matches_rank0 {
        return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            "rank-0 broadcast did not converge the bounded DDP lane".into(),
        ));
    }
    if !receipt.all_steps_match_baseline {
        let mismatch_detail = receipt
            .steps
            .iter()
            .find(|step| !step.rank0_matches_baseline || !step.rank1_matches_rank0)
            .map(|step| {
                format!(
                    "step={} rank0_baseline_max_model_delta={} rank1_rank0_max_model_delta={} rank0_baseline_max_optimizer_delta={} rank1_rank0_max_optimizer_delta={} rank0_model={} baseline_model={} rank1_model={}",
                    step.step_number,
                    step.rank0_baseline_max_model_delta,
                    step.rank1_rank0_max_model_delta,
                    step.rank0_baseline_max_optimizer_delta,
                    step.rank1_rank0_max_optimizer_delta,
                    step.rank0_step.model_state_digest_after,
                    step.baseline_step.model_state_digest_after,
                    step.rank1_step.model_state_digest_after,
                )
            })
            .unwrap_or_else(|| String::from("no step-level mismatch recorded"));
        return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            format!(
                "bounded DDP steps did not stay aligned with the non-parallel baseline: {mismatch_detail}"
            ),
        ));
    }
    Ok(())
}

fn bounded_distributed_group_receipt(
    world_size: usize,
) -> Result<Cs336A2DdpDistributedGroupReceipt, Cs336A2DdpIndividualParametersReceiptError> {
    if world_size != 2 {
        return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            format!(
                "bounded individual-parameter DDP receipt currently expects world_size=2, got {world_size}"
            ),
        ));
    }
    Ok(Cs336A2DdpDistributedGroupReceipt {
        world_size,
        rank_count: world_size,
        rank_device_labels: BTreeMap::from([
            (0usize, String::from("cpu:0")),
            (1usize, String::from("cpu:1")),
        ]),
        backend_family: String::from("bounded_reference_ddp"),
        communication_class: String::from("host_owned_gradient_sync"),
        topology_profile: String::from("two_rank_local_reference"),
        collective_support: BTreeMap::from([
            (
                String::from("broadcast"),
                String::from("owned_model_state_copy_from_rank0"),
            ),
            (
                String::from("individual_parameter_all_reduce"),
                String::from("host_owned_arithmetic_mean"),
            ),
            (
                String::from("bucketed_all_reduce"),
                String::from("not_implemented_in_this_issue"),
            ),
        ]),
    })
}

fn perturb_first_trainable_parameter(
    state: &mut ModuleStateDict,
) -> Result<(), Cs336A2DdpIndividualParametersReceiptError> {
    let Some((_, entry)) = state
        .entries
        .iter_mut()
        .find(|(_, entry)| entry.kind == ModuleStateEntryKind::Parameter && entry.requires_grad)
    else {
        return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            "missing trainable parameter to perturb before broadcast".into(),
        ));
    };
    let values = dense_tensor_values_mut(entry)?;
    if let Some(first) = values.first_mut() {
        *first += 0.25;
        return Ok(());
    }
    Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
        "first trainable parameter had no values".into(),
    ))
}

fn split_batch_across_ranks(
    batch: &Cs336A1ReferenceBatch,
    world_size: usize,
) -> Result<BTreeMap<usize, Cs336A1ReferenceBatch>, Cs336A2DdpIndividualParametersReceiptError> {
    if world_size == 0 || batch.batch_size % world_size != 0 {
        return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            "global batch size must divide evenly across the bounded world size".into(),
        ));
    }
    let local_batch_size = batch.batch_size / world_size;
    let mut local_batches = BTreeMap::new();
    for rank in 0..world_size {
        let row_start = rank * local_batch_size;
        let row_end = row_start + local_batch_size;
        let token_start = row_start * batch.context_length;
        let token_end = row_end * batch.context_length;
        local_batches.insert(
            rank,
            Cs336A1ReferenceBatch {
                iteration: batch.iteration,
                batch_size: local_batch_size,
                context_length: batch.context_length,
                start_positions: batch.start_positions[row_start..row_end].to_vec(),
                inputs: batch.inputs[token_start..token_end].to_vec(),
                targets: batch.targets[token_start..token_end].to_vec(),
            },
        );
    }
    Ok(local_batches)
}

struct SynchronizedGradientEntry {
    averaged_entry: ModuleStateEntry,
    receipt: Cs336A2DdpIndividualParameterSyncReceipt,
    differing_across_ranks: bool,
}

fn synchronize_gradient_entry(
    sync_index: usize,
    path: &str,
    rank0: &ModuleStateEntry,
    rank1: &ModuleStateEntry,
) -> Result<SynchronizedGradientEntry, Cs336A2DdpIndividualParametersReceiptError> {
    let rank0_values = dense_tensor_values(rank0)?.to_vec();
    let rank1_values = dense_tensor_values(rank1)?.to_vec();
    if rank0_values.len() != rank1_values.len() {
        return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            format!(
                "gradient length mismatch for `{path}`: rank0={} rank1={}",
                rank0_values.len(),
                rank1_values.len()
            ),
        ));
    }
    let averaged_values = rank0_values
        .iter()
        .zip(rank1_values.iter())
        .map(|(left, right)| (left + right) * 0.5)
        .collect::<Vec<_>>();
    let differing_across_ranks = rank0_values != rank1_values;
    Ok(SynchronizedGradientEntry {
        averaged_entry: ModuleStateEntry {
            path: path.to_string(),
            kind: ModuleStateEntryKind::Parameter,
            spec: rank0.spec.clone(),
            data: TensorData::F32(averaged_values.clone()),
            requires_grad: true,
            persistent: true,
        },
        receipt: Cs336A2DdpIndividualParameterSyncReceipt {
            sync_index,
            parameter_path: path.to_string(),
            gradient_element_count: averaged_values.len(),
            rank_gradient_digests: BTreeMap::from([
                (
                    0usize,
                    stable_json_digest(b"psion.cs336_a2.ddp.rank0_gradient", &rank0_values),
                ),
                (
                    1usize,
                    stable_json_digest(b"psion.cs336_a2.ddp.rank1_gradient", &rank1_values),
                ),
            ]),
            differing_across_ranks,
            synchronized_gradient_digest: stable_json_digest(
                b"psion.cs336_a2.ddp.synchronized_gradient",
                &averaged_values,
            ),
        },
        differing_across_ranks,
    })
}

fn dense_tensor_values(
    entry: &ModuleStateEntry,
) -> Result<&[f32], Cs336A2DdpIndividualParametersReceiptError> {
    match &entry.data {
        TensorData::F32(values) => Ok(values.as_slice()),
        other => Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
            format!(
                "expected dense f32 parameter entry for `{}`, found {other:?}",
                entry.path
            ),
        )),
    }
}

fn dense_tensor_values_mut(
    entry: &mut ModuleStateEntry,
) -> Result<&mut [f32], Cs336A2DdpIndividualParametersReceiptError> {
    match &mut entry.data {
        TensorData::F32(values) => Ok(values.as_mut_slice()),
        other => Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
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

fn gradient_state_dict_for_paths(
    trainer: &Cs336A1ReferenceTrainer,
    batch: &Cs336A1ReferenceBatch,
    base_loss: f32,
    parameter_paths: &[String],
) -> Result<ModuleStateDict, Cs336A2DdpIndividualParametersReceiptError> {
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

fn module_state_dict_max_abs_delta(
    left: &ModuleStateDict,
    right: &ModuleStateDict,
) -> Result<f32, Cs336A2DdpIndividualParametersReceiptError> {
    let mut max_delta = 0.0_f32;
    for (path, left_entry) in &left.entries {
        let right_entry = right.entries.get(path).ok_or_else(|| {
            Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(format!(
                "missing state entry `{path}` while computing DDP parity delta"
            ))
        })?;
        let left_values = dense_tensor_values(left_entry)?;
        let right_values = dense_tensor_values(right_entry)?;
        if left_values.len() != right_values.len() {
            return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
                format!("state entry `{path}` length mismatch while computing DDP parity delta"),
            ));
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
) -> Result<f32, Cs336A2DdpIndividualParametersReceiptError> {
    let mut max_delta = 0.0_f32;
    for (path, left_state) in left {
        let right_state = right.get(path).ok_or_else(|| {
            Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(format!(
                "missing optimizer state `{path}` while computing DDP parity delta"
            ))
        })?;
        let left_values = optimizer_state_values(left_state);
        let right_values = optimizer_state_values(right_state);
        if left_values.len() != right_values.len() {
            return Err(Cs336A2DdpIndividualParametersReceiptError::InvalidReceipt(
                format!(
                    "optimizer state `{path}` length mismatch while computing DDP parity delta"
                ),
            ));
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
        CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH,
        CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH,
        build_cs336_a2_ddp_individual_parameters_receipt,
        write_cs336_a2_ddp_individual_parameters_receipt,
    };

    #[test]
    fn ddp_individual_parameters_receipt_matches_non_parallel_baseline()
    -> Result<(), Box<dyn std::error::Error>> {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let receipt = build_cs336_a2_ddp_individual_parameters_receipt(repo_root)?;
        assert!(receipt.broadcast.broadcast_matches_rank0);
        assert!(receipt.all_steps_match_baseline);
        Ok(())
    }

    #[test]
    fn ddp_individual_parameters_writer_emits_json_fixture()
    -> Result<(), Box<dyn std::error::Error>> {
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
        let receipt = write_cs336_a2_ddp_individual_parameters_receipt(temp.path())?;
        let fixture_path = temp
            .path()
            .join(CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH);
        assert!(fixture_path.exists());
        let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&fixture_path)?)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(receipt.schema_version.as_str())
        );
        Ok(())
    }
}
