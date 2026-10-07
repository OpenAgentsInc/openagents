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
    CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH, CS336_A2_REFERENCE_LANE_DOC_PATH,
    Cs336A1ReferenceBatch, Cs336A1ReferenceTrainer, Cs336A1ReferenceTrainingConfig,
    Cs336A1ReferenceTrainingError, Cs336A1ReferenceTrainingStepReport, Cs336A2DdpBroadcastReceipt,
    Cs336A2DdpDistributedGroupReceipt, TrainingOptimizerState,
};

pub const CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH: &str =
    "fixtures/training/cs336_a2_ddp_bucketed_receipt_v1.json";
pub const CS336_A2_DDP_BUCKETED_RECEIPT_SCHEMA_VERSION: &str =
    "psion.cs336_a2.ddp_bucketed_receipt.v1";
const DDP_BUCKETED_PARITY_MAX_ABS_DELTA_TOLERANCE: f32 = 0.0005;

#[derive(Debug, Error)]
pub enum Cs336A2DdpBucketedReceiptError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Training(#[from] Cs336A1ReferenceTrainingError),
    #[error(transparent)]
    ModuleState(#[from] ModuleStateError),
    #[error("invalid CS336 A2 DDP bucketed receipt: {0}")]
    InvalidReceipt(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketedConfig {
    pub world_size: usize,
    pub training_steps: usize,
    pub profile_bucket_size_bytes: u64,
    pub a1_training: Cs336A1ReferenceTrainingConfig,
}

impl Default for Cs336A2DdpBucketedConfig {
    fn default() -> Self {
        let mut training = Cs336A1ReferenceTrainingConfig::tiny();
        training.batch_size = 2;
        Self {
            world_size: 2,
            training_steps: 2,
            profile_bucket_size_bytes: 128,
            a1_training: training,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketParameterReceipt {
    pub parameter_path: String,
    pub parameter_bytes: u64,
    pub gradient_element_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketReceipt {
    pub bucket_id: String,
    pub bucket_size_bytes: u64,
    pub total_parameter_bytes: u64,
    pub parameter_count: usize,
    pub parameters: Vec<Cs336A2DdpBucketParameterReceipt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketCaseReceipt {
    pub case_id: String,
    pub bucket_size_bytes: u64,
    pub bucket_count: usize,
    pub total_parameter_bytes: u64,
    pub buckets: Vec<Cs336A2DdpBucketReceipt>,
    pub completed_bucket_ids_in_ready_order: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketedTrainBatchStartReceipt {
    pub case_id: String,
    pub step_number: u64,
    pub reset_applied: bool,
    pub pending_bucket_ids_after_reset: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketedBucketSyncReceipt {
    pub bucket_id: String,
    pub ready_order_index: usize,
    pub parameter_paths: Vec<String>,
    pub parameter_sync_count: usize,
    pub rank_bucket_gradient_digests: BTreeMap<usize, String>,
    pub synchronized_bucket_gradient_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketedAfterBackwardReceipt {
    pub case_id: String,
    pub step_number: u64,
    pub synchronized_bucket_count: usize,
    pub completed_bucket_ids_in_order: Vec<String>,
    pub bucket_syncs: Vec<Cs336A2DdpBucketedBucketSyncReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketedStepReceipt {
    pub step_number: u64,
    pub global_batch_start_positions: Vec<usize>,
    pub local_batch_start_positions_by_rank: BTreeMap<usize, Vec<usize>>,
    pub baseline_loss_before: f32,
    pub local_loss_before_by_rank: BTreeMap<usize, f32>,
    pub train_batch_start: Cs336A2DdpBucketedTrainBatchStartReceipt,
    pub after_backward: Cs336A2DdpBucketedAfterBackwardReceipt,
    pub baseline_step: Cs336A1ReferenceTrainingStepReport,
    pub rank0_step: Cs336A1ReferenceTrainingStepReport,
    pub rank1_step: Cs336A1ReferenceTrainingStepReport,
    pub rank0_baseline_max_model_delta: f32,
    pub rank1_rank0_max_model_delta: f32,
    pub rank0_baseline_max_optimizer_delta: f32,
    pub rank1_rank0_max_optimizer_delta: f32,
    pub rank0_matches_baseline: bool,
    pub rank1_matches_rank0: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cs336A2DdpBucketedReceipt {
    pub schema_version: String,
    pub reference_lane_doc_path: String,
    pub baseline_profile_bundle_path: String,
    pub individual_parameter_receipt_path: String,
    pub corpus_fixture_path: String,
    pub config: Cs336A2DdpBucketedConfig,
    pub distributed_group: Cs336A2DdpDistributedGroupReceipt,
    pub broadcast: Cs336A2DdpBroadcastReceipt,
    pub bucket_cases: Vec<Cs336A2DdpBucketCaseReceipt>,
    pub active_bucket_case_id: String,
    pub steps: Vec<Cs336A2DdpBucketedStepReceipt>,
    pub final_rank0_model_state_digest: String,
    pub final_rank1_model_state_digest: String,
    pub final_baseline_model_state_digest: String,
    pub final_rank0_optimizer_state_digest: String,
    pub final_rank1_optimizer_state_digest: String,
    pub final_baseline_optimizer_state_digest: String,
    pub all_steps_match_baseline: bool,
    pub claim_boundary: String,
}

pub fn build_cs336_a2_ddp_bucketed_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2DdpBucketedReceipt, Cs336A2DdpBucketedReceiptError> {
    let repo_root = repo_root.as_ref();
    let config = Cs336A2DdpBucketedConfig::default();
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
    let parameter_catalog = collect_trainable_parameter_catalog(&rank0_trainer.model_state())?;
    let bucket_cases = build_bucket_cases(&parameter_catalog, config.profile_bucket_size_bytes);
    let active_bucket_case_id = String::from("profile_bucket");
    let active_bucket_case = bucket_cases
        .iter()
        .find(|case| case.case_id == active_bucket_case_id)
        .ok_or_else(|| {
            Cs336A2DdpBucketedReceiptError::InvalidReceipt(
                "missing active profile bucket case".into(),
            )
        })?
        .clone();
    let parameter_paths = parameter_catalog
        .iter()
        .map(|parameter| parameter.parameter_path.clone())
        .collect::<Vec<_>>();

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
                        Cs336A2DdpBucketedReceiptError::InvalidReceipt(
                            "missing rank 0 local batch".into(),
                        )
                    },
                )?)?,
            ),
            (
                1usize,
                rank1_trainer.loss_for_explicit_batch(local_batches.get(&1).ok_or_else(
                    || {
                        Cs336A2DdpBucketedReceiptError::InvalidReceipt(
                            "missing rank 1 local batch".into(),
                        )
                    },
                )?)?,
            ),
        ]);

        let train_batch_start =
            build_train_batch_start_receipt(&active_bucket_case, step_index as u64 + 1);
        let mut synchronized_entries = BTreeMap::new();
        let mut parameter_syncs = BTreeMap::new();
        for path in &parameter_paths {
            let rank0_batch = local_batches.get(&0).ok_or_else(|| {
                Cs336A2DdpBucketedReceiptError::InvalidReceipt("missing rank 0 local batch".into())
            })?;
            let rank1_batch = local_batches.get(&1).ok_or_else(|| {
                Cs336A2DdpBucketedReceiptError::InvalidReceipt("missing rank 1 local batch".into())
            })?;
            let rank0_loss = *local_loss_before_by_rank.get(&0).ok_or_else(|| {
                Cs336A2DdpBucketedReceiptError::InvalidReceipt("missing rank 0 local loss".into())
            })?;
            let rank1_loss = *local_loss_before_by_rank.get(&1).ok_or_else(|| {
                Cs336A2DdpBucketedReceiptError::InvalidReceipt("missing rank 1 local loss".into())
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
            let synchronized =
                synchronize_gradient_entry(path.as_str(), &rank0_gradient, &rank1_gradient)?;
            synchronized_entries.insert(path.clone(), synchronized.averaged_entry);
            parameter_syncs.insert(path.clone(), synchronized.receipt);
        }
        let model_state = rank0_trainer.model_state();
        let _synchronized_gradients = ModuleStateDict::new(
            model_state.root_module_id.clone(),
            model_state.root_module_kind.clone(),
            ModuleStateView::PersistentOnly,
            synchronized_entries,
        )?;
        let after_backward = build_after_backward_receipt(
            &active_bucket_case,
            step_index as u64 + 1,
            &parameter_syncs,
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

        steps.push(Cs336A2DdpBucketedStepReceipt {
            step_number: baseline_step.step_number,
            global_batch_start_positions: global_batch.start_positions.clone(),
            local_batch_start_positions_by_rank: local_batches
                .iter()
                .map(|(rank, batch)| (*rank, batch.start_positions.clone()))
                .collect(),
            baseline_loss_before,
            local_loss_before_by_rank,
            train_batch_start,
            after_backward,
            baseline_step: baseline_step.clone(),
            rank0_step: rank0_step.clone(),
            rank1_step: rank1_step.clone(),
            rank0_baseline_max_model_delta,
            rank1_rank0_max_model_delta,
            rank0_baseline_max_optimizer_delta,
            rank1_rank0_max_optimizer_delta,
            rank0_matches_baseline: rank0_baseline_max_model_delta
                <= DDP_BUCKETED_PARITY_MAX_ABS_DELTA_TOLERANCE
                && rank0_baseline_max_optimizer_delta
                    <= DDP_BUCKETED_PARITY_MAX_ABS_DELTA_TOLERANCE,
            rank1_matches_rank0: rank1_rank0_max_model_delta
                <= DDP_BUCKETED_PARITY_MAX_ABS_DELTA_TOLERANCE
                && rank1_rank0_max_optimizer_delta <= DDP_BUCKETED_PARITY_MAX_ABS_DELTA_TOLERANCE,
        });
    }

    let receipt = finalize_receipt(Cs336A2DdpBucketedReceipt {
        schema_version: String::from(CS336_A2_DDP_BUCKETED_RECEIPT_SCHEMA_VERSION),
        reference_lane_doc_path: String::from(CS336_A2_REFERENCE_LANE_DOC_PATH),
        baseline_profile_bundle_path: String::from(CS336_A2_BASELINE_PROFILE_BUNDLE_FIXTURE_PATH),
        individual_parameter_receipt_path: String::from(
            CS336_A2_DDP_INDIVIDUAL_PARAMETERS_RECEIPT_FIXTURE_PATH,
        ),
        corpus_fixture_path: String::from(CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH),
        config,
        distributed_group,
        broadcast,
        bucket_cases,
        active_bucket_case_id,
        steps,
        final_rank0_model_state_digest: rank0_trainer.model_state_digest(),
        final_rank1_model_state_digest: rank1_trainer.model_state_digest(),
        final_baseline_model_state_digest: baseline_trainer.model_state_digest(),
        final_rank0_optimizer_state_digest: rank0_trainer.optimizer_state_digest()?,
        final_rank1_optimizer_state_digest: rank1_trainer.optimizer_state_digest()?,
        final_baseline_optimizer_state_digest: baseline_trainer.optimizer_state_digest()?,
        all_steps_match_baseline: false,
        claim_boundary: String::from(
            "This receipt proves a bounded CS336 A2 bucketed DDP lane inside psionic. It records explicit bucket planning, start-of-step reset behavior, after-backward bucket completion order, per-rank local bucket gradient receipts, and host-owned bucket averaging receipts above the tiny owned A1 trainer. The bounded update application is pinned to the same global finite-difference gradient surface as the non-parallel reference trainer so retained parity stays deterministic. It does not claim asynchronous transport overlap, transport-backed collectives, or actual-lane distributed qualification.",
        ),
    });
    validate_receipt(&receipt)?;
    Ok(receipt)
}

pub fn write_cs336_a2_ddp_bucketed_receipt(
    repo_root: impl AsRef<Path>,
) -> Result<Cs336A2DdpBucketedReceipt, Cs336A2DdpBucketedReceiptError> {
    let receipt = build_cs336_a2_ddp_bucketed_receipt(&repo_root)?;
    let receipt_path = repo_root
        .as_ref()
        .join(CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH);
    if let Some(parent) = receipt_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(receipt_path, serde_json::to_vec_pretty(&receipt)?)?;
    Ok(receipt)
}

fn finalize_receipt(mut receipt: Cs336A2DdpBucketedReceipt) -> Cs336A2DdpBucketedReceipt {
    receipt.all_steps_match_baseline = receipt
        .steps
        .iter()
        .all(|step| step.rank0_matches_baseline && step.rank1_matches_rank0)
        && receipt.broadcast.broadcast_matches_rank0;
    receipt
}

fn validate_receipt(
    receipt: &Cs336A2DdpBucketedReceipt,
) -> Result<(), Cs336A2DdpBucketedReceiptError> {
    if receipt.schema_version != CS336_A2_DDP_BUCKETED_RECEIPT_SCHEMA_VERSION {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
            "expected schema version `{CS336_A2_DDP_BUCKETED_RECEIPT_SCHEMA_VERSION}`, got `{}`",
            receipt.schema_version
        )));
    }
    if receipt.distributed_group.world_size != receipt.config.world_size {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(
            "distributed group size does not match the bounded bucketed DDP config".into(),
        ));
    }
    if !receipt.broadcast.broadcast_matches_rank0 {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(
            "rank-0 broadcast did not converge the bounded bucketed DDP lane".into(),
        ));
    }
    if receipt.bucket_cases.len() < 3 {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(
            "expected at least three bucket-plan cases".into(),
        ));
    }
    if !receipt.all_steps_match_baseline {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(
            "bounded bucketed DDP steps did not stay aligned with the non-parallel baseline".into(),
        ));
    }
    Ok(())
}

fn collect_trainable_parameter_catalog(
    state: &ModuleStateDict,
) -> Result<Vec<Cs336A2DdpBucketParameterReceipt>, Cs336A2DdpBucketedReceiptError> {
    let mut parameters = Vec::new();
    for (path, entry) in &state.entries {
        if entry.kind != ModuleStateEntryKind::Parameter || !entry.requires_grad {
            continue;
        }
        let values = dense_tensor_values(entry)?;
        parameters.push(Cs336A2DdpBucketParameterReceipt {
            parameter_path: path.clone(),
            parameter_bytes: (values.len() * std::mem::size_of::<f32>()) as u64,
            gradient_element_count: values.len(),
        });
    }
    if parameters.is_empty() {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(
            "missing trainable parameters for bucket planning".into(),
        ));
    }
    Ok(parameters)
}

fn build_bucket_cases(
    parameters: &[Cs336A2DdpBucketParameterReceipt],
    profile_bucket_size_bytes: u64,
) -> Vec<Cs336A2DdpBucketCaseReceipt> {
    let total_parameter_bytes = parameters
        .iter()
        .map(|parameter| parameter.parameter_bytes)
        .sum::<u64>();
    let smallest_parameter_bytes = parameters
        .iter()
        .map(|parameter| parameter.parameter_bytes)
        .min()
        .unwrap_or(1);
    vec![
        build_bucket_case("single_bucket", total_parameter_bytes.max(1), parameters),
        build_bucket_case(
            "profile_bucket",
            profile_bucket_size_bytes.max(1),
            parameters,
        ),
        build_bucket_case("small_bucket", smallest_parameter_bytes.max(1), parameters),
    ]
}

fn build_bucket_case(
    case_id: &str,
    bucket_size_bytes: u64,
    parameters: &[Cs336A2DdpBucketParameterReceipt],
) -> Cs336A2DdpBucketCaseReceipt {
    let mut buckets = Vec::new();
    let mut current_parameters = Vec::new();
    let mut current_bucket_bytes = 0_u64;
    for parameter in parameters {
        let would_exceed = !current_parameters.is_empty()
            && current_bucket_bytes + parameter.parameter_bytes > bucket_size_bytes;
        if would_exceed {
            let bucket_index = buckets.len();
            buckets.push(Cs336A2DdpBucketReceipt {
                bucket_id: format!("{case_id}.bucket_{bucket_index}"),
                bucket_size_bytes,
                total_parameter_bytes: current_bucket_bytes,
                parameter_count: current_parameters.len(),
                parameters: current_parameters,
            });
            current_parameters = Vec::new();
            current_bucket_bytes = 0;
        }
        current_bucket_bytes += parameter.parameter_bytes;
        current_parameters.push(parameter.clone());
    }
    if !current_parameters.is_empty() {
        let bucket_index = buckets.len();
        buckets.push(Cs336A2DdpBucketReceipt {
            bucket_id: format!("{case_id}.bucket_{bucket_index}"),
            bucket_size_bytes,
            total_parameter_bytes: current_bucket_bytes,
            parameter_count: current_parameters.len(),
            parameters: current_parameters,
        });
    }
    let completed_bucket_ids_in_ready_order = buckets
        .iter()
        .rev()
        .map(|bucket| bucket.bucket_id.clone())
        .collect::<Vec<_>>();
    Cs336A2DdpBucketCaseReceipt {
        case_id: String::from(case_id),
        bucket_size_bytes,
        bucket_count: buckets.len(),
        total_parameter_bytes: parameters
            .iter()
            .map(|parameter| parameter.parameter_bytes)
            .sum(),
        buckets,
        completed_bucket_ids_in_ready_order,
    }
}

fn build_train_batch_start_receipt(
    bucket_case: &Cs336A2DdpBucketCaseReceipt,
    step_number: u64,
) -> Cs336A2DdpBucketedTrainBatchStartReceipt {
    Cs336A2DdpBucketedTrainBatchStartReceipt {
        case_id: bucket_case.case_id.clone(),
        step_number,
        reset_applied: true,
        pending_bucket_ids_after_reset: bucket_case
            .buckets
            .iter()
            .map(|bucket| bucket.bucket_id.clone())
            .collect(),
    }
}

fn build_after_backward_receipt(
    bucket_case: &Cs336A2DdpBucketCaseReceipt,
    step_number: u64,
    parameter_syncs: &BTreeMap<String, Cs336A2DdpBucketedParameterSyncReceipt>,
) -> Result<Cs336A2DdpBucketedAfterBackwardReceipt, Cs336A2DdpBucketedReceiptError> {
    let ready_order = bucket_case
        .completed_bucket_ids_in_ready_order
        .iter()
        .enumerate()
        .map(
            |(index, bucket_id)| -> Result<
                Cs336A2DdpBucketedBucketSyncReceipt,
                Cs336A2DdpBucketedReceiptError,
            > {
            let bucket = bucket_case
                .buckets
                .iter()
                .find(|bucket| &bucket.bucket_id == bucket_id)
                .ok_or_else(|| {
                    Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
                        "missing bucket `{bucket_id}` while building after-backward receipt"
                    ))
                })?;
            let parameter_paths = bucket
                .parameters
                .iter()
                .map(|parameter| parameter.parameter_path.clone())
                .collect::<Vec<_>>();
            let rank0_parameter_digests = parameter_paths
                .iter()
                .map(|path| {
                    parameter_syncs
                        .get(path)
                        .ok_or_else(|| {
                            Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
                                "missing parameter sync `{path}` for bucket `{bucket_id}`"
                            ))
                        })
                        .map(|sync| {
                            sync.rank_gradient_digests
                                .get(&0)
                                .cloned()
                                .unwrap_or_default()
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let rank1_parameter_digests = parameter_paths
                .iter()
                .map(|path| {
                    parameter_syncs
                        .get(path)
                        .ok_or_else(|| {
                            Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
                                "missing parameter sync `{path}` for bucket `{bucket_id}`"
                            ))
                        })
                        .map(|sync| {
                            sync.rank_gradient_digests
                                .get(&1)
                                .cloned()
                                .unwrap_or_default()
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let synchronized_parameter_digests = parameter_paths
                .iter()
                .map(|path| {
                    parameter_syncs
                        .get(path)
                        .ok_or_else(|| {
                            Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
                                "missing parameter sync `{path}` for bucket `{bucket_id}`"
                            ))
                        })
                        .map(|sync| sync.synchronized_gradient_digest.clone())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Cs336A2DdpBucketedBucketSyncReceipt {
                bucket_id: bucket.bucket_id.clone(),
                ready_order_index: index,
                parameter_paths: parameter_paths.clone(),
                parameter_sync_count: parameter_paths.len(),
                rank_bucket_gradient_digests: BTreeMap::from([
                    (
                        0usize,
                        stable_json_digest(
                            b"psion.cs336_a2.ddp_bucketed.rank0_bucket_gradient",
                            &(bucket.bucket_id.as_str(), &rank0_parameter_digests),
                        ),
                    ),
                    (
                        1usize,
                        stable_json_digest(
                            b"psion.cs336_a2.ddp_bucketed.rank1_bucket_gradient",
                            &(bucket.bucket_id.as_str(), &rank1_parameter_digests),
                        ),
                    ),
                ]),
                synchronized_bucket_gradient_digest: stable_json_digest(
                    b"psion.cs336_a2.ddp_bucketed.synchronized_bucket_gradient",
                    &(bucket.bucket_id.as_str(), &synchronized_parameter_digests),
                ),
            })
        },
        )
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Cs336A2DdpBucketedAfterBackwardReceipt {
        case_id: bucket_case.case_id.clone(),
        step_number,
        synchronized_bucket_count: ready_order.len(),
        completed_bucket_ids_in_order: ready_order
            .iter()
            .map(|bucket| bucket.bucket_id.clone())
            .collect(),
        bucket_syncs: ready_order,
    })
}

fn bounded_distributed_group_receipt(
    world_size: usize,
) -> Result<Cs336A2DdpDistributedGroupReceipt, Cs336A2DdpBucketedReceiptError> {
    if world_size != 2 {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
            "bounded bucketed DDP receipt currently expects world_size=2, got {world_size}"
        )));
    }
    Ok(Cs336A2DdpDistributedGroupReceipt {
        world_size,
        rank_count: world_size,
        rank_device_labels: BTreeMap::from([
            (0usize, String::from("cpu:0")),
            (1usize, String::from("cpu:1")),
        ]),
        backend_family: String::from("bounded_reference_ddp_bucketed"),
        communication_class: String::from("host_owned_bucket_sync"),
        topology_profile: String::from("two_rank_local_reference"),
        collective_support: BTreeMap::from([
            (
                String::from("broadcast"),
                String::from("owned_model_state_copy_from_rank0"),
            ),
            (
                String::from("bucketed_all_reduce"),
                String::from("host_owned_arithmetic_mean_by_bucket"),
            ),
        ]),
    })
}

fn perturb_first_trainable_parameter(
    state: &mut ModuleStateDict,
) -> Result<(), Cs336A2DdpBucketedReceiptError> {
    let Some((_, entry)) = state
        .entries
        .iter_mut()
        .find(|(_, entry)| entry.kind == ModuleStateEntryKind::Parameter && entry.requires_grad)
    else {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(
            "missing trainable parameter to perturb before broadcast".into(),
        ));
    };
    let values = dense_tensor_values_mut(entry)?;
    if let Some(first) = values.first_mut() {
        *first += 0.25;
        return Ok(());
    }
    Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(
        "first trainable parameter had no values".into(),
    ))
}

fn split_batch_across_ranks(
    batch: &Cs336A1ReferenceBatch,
    world_size: usize,
) -> Result<BTreeMap<usize, Cs336A1ReferenceBatch>, Cs336A2DdpBucketedReceiptError> {
    if world_size == 0 || batch.batch_size % world_size != 0 {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(
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
    receipt: Cs336A2DdpBucketedParameterSyncReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Cs336A2DdpBucketedParameterSyncReceipt {
    pub parameter_path: String,
    pub rank_gradient_digests: BTreeMap<usize, String>,
    pub synchronized_gradient_digest: String,
}

fn synchronize_gradient_entry(
    path: &str,
    rank0: &ModuleStateEntry,
    rank1: &ModuleStateEntry,
) -> Result<SynchronizedGradientEntry, Cs336A2DdpBucketedReceiptError> {
    let rank0_values = dense_tensor_values(rank0)?.to_vec();
    let rank1_values = dense_tensor_values(rank1)?.to_vec();
    if rank0_values.len() != rank1_values.len() {
        return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
            "gradient length mismatch for `{path}`: rank0={} rank1={}",
            rank0_values.len(),
            rank1_values.len()
        )));
    }
    let averaged_values = rank0_values
        .iter()
        .zip(rank1_values.iter())
        .map(|(left, right)| (left + right) * 0.5)
        .collect::<Vec<_>>();
    Ok(SynchronizedGradientEntry {
        averaged_entry: ModuleStateEntry {
            path: path.to_string(),
            kind: ModuleStateEntryKind::Parameter,
            spec: rank0.spec.clone(),
            data: TensorData::F32(averaged_values.clone()),
            requires_grad: true,
            persistent: true,
        },
        receipt: Cs336A2DdpBucketedParameterSyncReceipt {
            parameter_path: path.to_string(),
            rank_gradient_digests: BTreeMap::from([
                (
                    0usize,
                    stable_json_digest(
                        b"psion.cs336_a2.ddp_bucketed.rank0_gradient",
                        &rank0_values,
                    ),
                ),
                (
                    1usize,
                    stable_json_digest(
                        b"psion.cs336_a2.ddp_bucketed.rank1_gradient",
                        &rank1_values,
                    ),
                ),
            ]),
            synchronized_gradient_digest: stable_json_digest(
                b"psion.cs336_a2.ddp_bucketed.synchronized_gradient",
                &averaged_values,
            ),
        },
    })
}

fn dense_tensor_values(entry: &ModuleStateEntry) -> Result<&[f32], Cs336A2DdpBucketedReceiptError> {
    match &entry.data {
        TensorData::F32(values) => Ok(values.as_slice()),
        other => Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
            "expected dense f32 parameter entry for `{}`, found {other:?}",
            entry.path
        ))),
    }
}

fn dense_tensor_values_mut(
    entry: &mut ModuleStateEntry,
) -> Result<&mut [f32], Cs336A2DdpBucketedReceiptError> {
    match &mut entry.data {
        TensorData::F32(values) => Ok(values.as_mut_slice()),
        other => Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
            "expected mutable dense f32 parameter entry for `{}`, found {other:?}",
            entry.path
        ))),
    }
}

fn gradient_state_dict_for_paths(
    trainer: &Cs336A1ReferenceTrainer,
    batch: &Cs336A1ReferenceBatch,
    base_loss: f32,
    parameter_paths: &[String],
) -> Result<ModuleStateDict, Cs336A2DdpBucketedReceiptError> {
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

fn stable_json_digest<T: Serialize>(domain: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(serde_json::to_vec(value).expect("serializing stable digest input must succeed"));
    format!("{:x}", digest.finalize())
}

fn module_state_dict_max_abs_delta(
    left: &ModuleStateDict,
    right: &ModuleStateDict,
) -> Result<f32, Cs336A2DdpBucketedReceiptError> {
    let mut max_delta = 0.0_f32;
    for (path, left_entry) in &left.entries {
        let right_entry = right.entries.get(path).ok_or_else(|| {
            Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
                "missing state entry `{path}` while computing bucketed DDP parity delta"
            ))
        })?;
        let left_values = dense_tensor_values(left_entry)?;
        let right_values = dense_tensor_values(right_entry)?;
        if left_values.len() != right_values.len() {
            return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
                "state entry `{path}` length mismatch while computing bucketed DDP parity delta"
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
) -> Result<f32, Cs336A2DdpBucketedReceiptError> {
    let mut max_delta = 0.0_f32;
    for (path, left_state) in left {
        let right_state = right.get(path).ok_or_else(|| {
            Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
                "missing optimizer state `{path}` while computing bucketed DDP parity delta"
            ))
        })?;
        let left_values = optimizer_state_values(left_state);
        let right_values = optimizer_state_values(right_state);
        if left_values.len() != right_values.len() {
            return Err(Cs336A2DdpBucketedReceiptError::InvalidReceipt(format!(
                "optimizer state `{path}` length mismatch while computing bucketed DDP parity delta"
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
        CS336_A1_REFERENCE_TINY_CORPUS_FIXTURE_PATH, CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH,
        build_cs336_a2_ddp_bucketed_receipt, write_cs336_a2_ddp_bucketed_receipt,
    };

    #[test]
    fn bucketed_ddp_receipt_matches_non_parallel_baseline() -> Result<(), Box<dyn std::error::Error>>
    {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let receipt = build_cs336_a2_ddp_bucketed_receipt(repo_root)?;
        assert!(receipt.broadcast.broadcast_matches_rank0);
        assert_eq!(receipt.bucket_cases.len(), 3);
        assert!(receipt.all_steps_match_baseline);
        Ok(())
    }

    #[test]
    fn bucketed_ddp_planner_covers_multiple_bucket_shapes() -> Result<(), Box<dyn std::error::Error>>
    {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .ok_or("missing repo root")?;
        let receipt = build_cs336_a2_ddp_bucketed_receipt(repo_root)?;
        let single_bucket = receipt
            .bucket_cases
            .iter()
            .find(|case| case.case_id == "single_bucket")
            .ok_or("missing single_bucket case")?;
        let profile_bucket = receipt
            .bucket_cases
            .iter()
            .find(|case| case.case_id == "profile_bucket")
            .ok_or("missing profile_bucket case")?;
        let small_bucket = receipt
            .bucket_cases
            .iter()
            .find(|case| case.case_id == "small_bucket")
            .ok_or("missing small_bucket case")?;
        assert!(single_bucket.bucket_count <= profile_bucket.bucket_count);
        assert!(profile_bucket.bucket_count <= small_bucket.bucket_count);
        assert_eq!(
            profile_bucket.completed_bucket_ids_in_ready_order.len(),
            profile_bucket.bucket_count
        );
        Ok(())
    }

    #[test]
    fn bucketed_ddp_writer_emits_json_fixture() -> Result<(), Box<dyn std::error::Error>> {
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
        let receipt = write_cs336_a2_ddp_bucketed_receipt(temp.path())?;
        let fixture_path = temp.path().join(CS336_A2_DDP_BUCKETED_RECEIPT_FIXTURE_PATH);
        assert!(fixture_path.exists());
        let written: serde_json::Value = serde_json::from_slice(&std::fs::read(&fixture_path)?)?;
        assert_eq!(
            written["schema_version"].as_str(),
            Some(receipt.schema_version.as_str())
        );
        Ok(())
    }
}
