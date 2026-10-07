use std::{
    fs,
    path::{Path, PathBuf},
};

use psionic_core::PsionicRefusal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    execute_parameter_golf_distributed_8xh100_runtime_bootstrap,
    execute_parameter_golf_distributed_8xh100_train_step,
    parameter_golf_distributed_8xh100_runtime_bootstrap_receipt_path,
    parameter_golf_distributed_8xh100_train_step_receipt_path,
    ParameterGolfDistributed8xH100BringupReport,
    ParameterGolfDistributed8xH100RuntimeBootstrapDisposition,
    ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
    ParameterGolfDistributed8xH100TrainStepReceipt,
    ParameterGolfDistributedLiveVisualizationWriter, ParameterGolfScoreFirstTttConfig,
};

/// Stable schema version for the generic dense-rank runtime reference contract.
pub const DENSE_RANK_RUNTIME_REFERENCE_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.dense_rank_runtime_reference_contract.v1";
/// Stable fixture path for the generic dense-rank runtime reference contract.
pub const DENSE_RANK_RUNTIME_REFERENCE_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/dense_rank_runtime_reference_contract_v1.json";
/// Stable checker path for the dense-rank runtime reference contract.
pub const DENSE_RANK_RUNTIME_REFERENCE_CHECK_SCRIPT_PATH: &str =
    "scripts/check-dense-rank-runtime-reference-contract.sh";
/// Stable reference doc path for the dense-rank runtime module.
pub const DENSE_RANK_RUNTIME_REFERENCE_DOC_PATH: &str = "docs/DENSE_RANK_RUNTIME_REFERENCE.md";
/// Stable runtime family id promoted out of the PGOLF lane.
pub const DENSE_RANK_RUNTIME_FAMILY_ID: &str = "psionic.dense_rank_runtime.cuda.v1";
/// Stable generic consumer lane used by the reference contract.
pub const DENSE_RANK_RUNTIME_REFERENCE_LANE_ID: &str =
    "psion.cross_provider_pretraining_dense_reference";
/// Stable current PGOLF consumer lane id using the shared runtime.
pub const DENSE_RANK_RUNTIME_PARAMETER_GOLF_CONSUMER_LANE_ID: &str =
    "parameter_golf.distributed_8xh100";
/// Stable checkpoint family bound by the generic runtime reference contract.
pub const DENSE_RANK_RUNTIME_REFERENCE_CHECKPOINT_FAMILY: &str = "psion.cross_provider.pretrain.v1";
/// Stable dataset family bound by the generic runtime reference contract.
pub const DENSE_RANK_RUNTIME_REFERENCE_DATASET_FAMILY: &str =
    "psion.curated_pretrain.dataset_family.v1";
/// Stable validation hook id exposed by the generic runtime layer.
pub const DENSE_RANK_RUNTIME_REFERENCE_VALIDATION_HOOK_ID: &str =
    "dense_rank_runtime.validation.post_train_eval";
/// Stable checkpoint hook id exposed by the generic runtime layer.
pub const DENSE_RANK_RUNTIME_REFERENCE_CHECKPOINT_HOOK_ID: &str =
    "dense_rank_runtime.checkpoint.materialization";

/// Errors surfaced while promoting the PGOLF runtime into the generic dense-rank layer.
#[derive(Debug, Error)]
pub enum DenseRankRuntimeError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    RuntimeBootstrap(#[from] crate::ParameterGolfDistributed8xH100RuntimeBootstrapError),
    #[error(transparent)]
    TrainStep(#[from] crate::ParameterGolfDistributed8xH100TrainStepError),
}

/// Generic dense-rank runtime identity above any one consumer lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseRankRuntimeIdentity {
    /// Stable runtime family id.
    pub runtime_family_id: String,
    /// Stable consumer lane id.
    pub consumer_lane_id: String,
    /// Stable checkpoint family.
    pub checkpoint_family: String,
    /// Stable dataset family.
    pub dataset_family_id: String,
    /// Stable requested backend.
    pub requested_backend: String,
    /// Stable world size.
    pub world_size: usize,
}

/// Validation hook contract exposed by the generic dense-rank runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseRankValidationHookContract {
    /// Stable hook id.
    pub hook_id: String,
    /// Current validation posture.
    pub posture: String,
    /// Honest detail for the current boundary.
    pub detail: String,
}

/// Checkpoint hook contract exposed by the generic dense-rank runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseRankCheckpointHookContract {
    /// Stable hook id.
    pub hook_id: String,
    /// Current checkpoint posture.
    pub posture: String,
    /// Honest detail for the current boundary.
    pub detail: String,
}

/// Generic bootstrap receipt promoted above the PGOLF lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DenseRankRuntimeBootstrapReceipt {
    /// Stable schema version.
    pub schema_version: u32,
    /// Generic runtime identity.
    pub runtime: DenseRankRuntimeIdentity,
    /// Stable run id.
    pub run_id: String,
    /// Bring-up report path.
    pub bringup_report_path: String,
    /// Bring-up report digest.
    pub bringup_report_digest: String,
    /// Runtime payload path.
    pub runtime_payload_path: String,
    /// Runtime manifest path.
    pub runtime_manifest_path: String,
    /// Successful rank count.
    pub successful_rank_count: usize,
    /// Final disposition.
    pub disposition: String,
    /// Primary refusal when one exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refusal: Option<PsionicRefusal>,
    /// Stable digest over the generic bootstrap receipt.
    pub receipt_digest: String,
}

impl DenseRankRuntimeBootstrapReceipt {
    /// Returns the stable digest over the generic bootstrap receipt.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_dense_rank_runtime_bootstrap_receipt|",
            &digestible,
        )
    }
}

/// Generic train-step receipt promoted above the PGOLF lane.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DenseRankRuntimeTrainStepReceipt {
    /// Stable schema version.
    pub schema_version: u32,
    /// Generic runtime identity.
    pub runtime: DenseRankRuntimeIdentity,
    /// Stable run id.
    pub run_id: String,
    /// Runtime bootstrap receipt path.
    pub runtime_bootstrap_receipt_path: String,
    /// Runtime bootstrap receipt digest.
    pub runtime_bootstrap_receipt_digest: String,
    /// Measurements path.
    pub measurements_path: String,
    /// Distributed throughput receipt path.
    pub distributed_receipt_path: String,
    /// Lane-specific train-step receipt path.
    pub train_step_receipt_path: String,
    /// Mean train loss.
    pub mean_train_loss: f32,
    /// Global train tokens.
    pub train_tokens: u64,
    /// Observed step wallclock.
    pub observed_step_ms: u64,
    /// Gradient synchronization wallclock.
    pub gradient_sync_ms: u64,
    /// Optimizer-step wallclock.
    pub optimizer_step_ms: u64,
    /// Validation hook contract.
    pub validation_hook: DenseRankValidationHookContract,
    /// Checkpoint hook contract.
    pub checkpoint_hook: DenseRankCheckpointHookContract,
    /// Stable digest over the generic train-step receipt.
    pub receipt_digest: String,
}

impl DenseRankRuntimeTrainStepReceipt {
    /// Returns the stable digest over the generic train-step receipt.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_dense_rank_runtime_train_step_receipt|",
            &digestible,
        )
    }
}

/// Generic dense-rank execution receipt that PGOLF now consumes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DenseRankRuntimeExecutionReceipt {
    /// Stable schema version.
    pub schema_version: u32,
    /// Generic runtime identity.
    pub runtime: DenseRankRuntimeIdentity,
    /// Stable run id.
    pub run_id: String,
    /// Generic bootstrap receipt.
    pub bootstrap: DenseRankRuntimeBootstrapReceipt,
    /// Generic train-step receipt.
    pub train_step: DenseRankRuntimeTrainStepReceipt,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the execution receipt.
    pub receipt_digest: String,
}

impl DenseRankRuntimeExecutionReceipt {
    /// Returns the stable digest over the execution receipt.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.receipt_digest.clear();
        stable_digest(
            b"psionic_dense_rank_runtime_execution_receipt|",
            &digestible,
        )
    }
}

/// Aggregate generic runtime outcome that still exposes the PGOLF consumer receipts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterGolfBackedDenseRankRuntimeOutcome {
    /// Lane-specific bootstrap receipt path.
    pub bootstrap_receipt_path: String,
    /// Lane-specific bootstrap receipt.
    pub bootstrap_receipt: ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
    /// Lane-specific train-step receipt path.
    pub train_step_receipt_path: String,
    /// Lane-specific train-step receipt.
    pub train_step_receipt: ParameterGolfDistributed8xH100TrainStepReceipt,
    /// Generic dense-rank execution receipt path.
    pub dense_rank_execution_receipt_path: String,
    /// Generic dense-rank execution receipt.
    pub dense_rank_execution_receipt: DenseRankRuntimeExecutionReceipt,
}

/// Generic dense-rank runtime reference contract promoted above PGOLF.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenseRankRuntimeReferenceContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Generic runtime identity.
    pub runtime: DenseRankRuntimeIdentity,
    /// Validation hook contract.
    pub validation_hook: DenseRankValidationHookContract,
    /// Checkpoint hook contract.
    pub checkpoint_hook: DenseRankCheckpointHookContract,
    /// Stable generic execution receipt name.
    pub generic_execution_receipt_name: String,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the reference contract.
    pub contract_digest: String,
}

impl DenseRankRuntimeReferenceContract {
    /// Returns the stable digest over the reference contract.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut digestible = self.clone();
        digestible.contract_digest.clear();
        stable_digest(
            b"psionic_dense_rank_runtime_reference_contract|",
            &digestible,
        )
    }
}

/// Returns the canonical generic dense-rank runtime reference contract.
#[must_use]
pub fn dense_rank_runtime_reference_contract() -> DenseRankRuntimeReferenceContract {
    let mut contract = DenseRankRuntimeReferenceContract {
        schema_version: String::from(DENSE_RANK_RUNTIME_REFERENCE_CONTRACT_SCHEMA_VERSION),
        runtime: dense_rank_runtime_reference_identity(),
        validation_hook: DenseRankValidationHookContract {
            hook_id: String::from(DENSE_RANK_RUNTIME_REFERENCE_VALIDATION_HOOK_ID),
            posture: String::from("lane_owned_post_train_validation_deferred"),
            detail: String::from(
                "The generic dense-rank runtime now owns the validation hook contract even though the promoted PGOLF consumer still stops after one measured train step.",
            ),
        },
        checkpoint_hook: DenseRankCheckpointHookContract {
            hook_id: String::from(DENSE_RANK_RUNTIME_REFERENCE_CHECKPOINT_HOOK_ID),
            posture: String::from("checkpoint_contract_owned_no_materialized_dense_checkpoint_yet"),
            detail: String::from(
                "The generic dense-rank runtime now owns the checkpoint hook contract even though the promoted PGOLF consumer does not yet emit a dense distributed checkpoint shard set.",
            ),
        },
        generic_execution_receipt_name: String::from("dense_rank_runtime_execution_receipt_v1.json"),
        claim_boundary: String::from(
            "This reference contract promotes the CUDA dense-rank runtime substrate above Parameter Golf. It does not claim mixed-backend dense training, distributed checkpoint closure, or post-train validation closure by itself.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract
}

/// Writes the canonical dense-rank runtime reference contract fixture.
pub fn write_dense_rank_runtime_reference_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), DenseRankRuntimeError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| DenseRankRuntimeError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&dense_rank_runtime_reference_contract())?;
    fs::write(output_path, bytes).map_err(|error| DenseRankRuntimeError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

/// Returns the generic runtime identity used by the reference contract.
#[must_use]
pub fn dense_rank_runtime_reference_identity() -> DenseRankRuntimeIdentity {
    DenseRankRuntimeIdentity {
        runtime_family_id: String::from(DENSE_RANK_RUNTIME_FAMILY_ID),
        consumer_lane_id: String::from(DENSE_RANK_RUNTIME_REFERENCE_LANE_ID),
        checkpoint_family: String::from(DENSE_RANK_RUNTIME_REFERENCE_CHECKPOINT_FAMILY),
        dataset_family_id: String::from(DENSE_RANK_RUNTIME_REFERENCE_DATASET_FAMILY),
        requested_backend: String::from("nccl"),
        world_size: 8,
    }
}

/// Returns the generic runtime identity used by the current PGOLF consumer.
#[must_use]
pub fn parameter_golf_dense_rank_runtime_identity() -> DenseRankRuntimeIdentity {
    DenseRankRuntimeIdentity {
        consumer_lane_id: String::from(DENSE_RANK_RUNTIME_PARAMETER_GOLF_CONSUMER_LANE_ID),
        ..dense_rank_runtime_reference_identity()
    }
}

/// Returns the canonical generic dense-rank execution receipt path beside the bring-up report.
#[must_use]
pub fn dense_rank_runtime_execution_receipt_path(
    root: &Path,
    bringup_report_path: &str,
) -> PathBuf {
    let resolved = root.join(bringup_report_path);
    match resolved.parent() {
        Some(parent) => parent.join("dense_rank_runtime_execution_receipt_v1.json"),
        None => root.join("dense_rank_runtime_execution_receipt_v1.json"),
    }
}

/// Executes the promoted dense-rank runtime substrate using the current PGOLF CUDA 8xH100 consumer.
pub fn execute_parameter_golf_backed_dense_rank_runtime(
    root: &Path,
    manifest_path: &Path,
    run_id: &str,
    bringup_report_path: &Path,
    bringup_report: &ParameterGolfDistributed8xH100BringupReport,
    validation_eval_mode: &crate::ParameterGolfValidationEvalMode,
    validation_batch_sequences: u64,
    score_first_ttt: Option<&ParameterGolfScoreFirstTttConfig>,
    mut live_visualization_writer: Option<&mut ParameterGolfDistributedLiveVisualizationWriter>,
) -> Result<ParameterGolfBackedDenseRankRuntimeOutcome, DenseRankRuntimeError> {
    let bootstrap_receipt = execute_parameter_golf_distributed_8xh100_runtime_bootstrap(
        root,
        manifest_path,
        run_id,
        bringup_report_path,
        bringup_report,
        live_visualization_writer.as_deref_mut(),
    )?;
    let bringup_relpath = bringup_report_path
        .strip_prefix(root)
        .unwrap_or(bringup_report_path)
        .display()
        .to_string();
    let bootstrap_receipt_path =
        parameter_golf_distributed_8xh100_runtime_bootstrap_receipt_path(root, &bringup_relpath);
    let train_step_receipt = execute_parameter_golf_distributed_8xh100_train_step(
        root,
        manifest_path,
        run_id,
        bringup_report_path,
        bringup_report,
        &bootstrap_receipt_path,
        &bootstrap_receipt,
        validation_eval_mode,
        validation_batch_sequences,
        score_first_ttt,
        live_visualization_writer.as_deref_mut(),
    )?;
    let train_step_receipt_path =
        parameter_golf_distributed_8xh100_train_step_receipt_path(root, &bringup_relpath);
    let dense_rank_execution_receipt_path =
        dense_rank_runtime_execution_receipt_path(root, &bringup_relpath);
    let dense_rank_execution_receipt = dense_rank_runtime_execution_receipt_from_parameter_golf(
        run_id,
        bringup_report_path,
        bringup_report,
        &bootstrap_receipt_path,
        &bootstrap_receipt,
        &train_step_receipt_path,
        &train_step_receipt,
    );
    if let Some(parent) = dense_rank_execution_receipt_path.parent() {
        fs::create_dir_all(parent).map_err(|error| DenseRankRuntimeError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&dense_rank_execution_receipt)?;
    fs::write(&dense_rank_execution_receipt_path, bytes).map_err(|error| {
        DenseRankRuntimeError::Write {
            path: dense_rank_execution_receipt_path.display().to_string(),
            error,
        }
    })?;
    Ok(ParameterGolfBackedDenseRankRuntimeOutcome {
        bootstrap_receipt_path: bootstrap_receipt_path.display().to_string(),
        bootstrap_receipt,
        train_step_receipt_path: train_step_receipt_path.display().to_string(),
        train_step_receipt,
        dense_rank_execution_receipt_path: dense_rank_execution_receipt_path.display().to_string(),
        dense_rank_execution_receipt,
    })
}

fn dense_rank_runtime_execution_receipt_from_parameter_golf(
    run_id: &str,
    bringup_report_path: &Path,
    bringup_report: &ParameterGolfDistributed8xH100BringupReport,
    bootstrap_receipt_path: &Path,
    bootstrap_receipt: &ParameterGolfDistributed8xH100RuntimeBootstrapReceipt,
    train_step_receipt_path: &Path,
    train_step_receipt: &ParameterGolfDistributed8xH100TrainStepReceipt,
) -> DenseRankRuntimeExecutionReceipt {
    let runtime = parameter_golf_dense_rank_runtime_identity();
    let mut bootstrap = DenseRankRuntimeBootstrapReceipt {
        schema_version: 1,
        runtime: runtime.clone(),
        run_id: String::from(run_id),
        bringup_report_path: bringup_report_path.display().to_string(),
        bringup_report_digest: bringup_report.report_digest.clone(),
        runtime_payload_path: bootstrap_receipt.runtime_payload_path.clone(),
        runtime_manifest_path: bootstrap_receipt.runtime_manifest_path.clone(),
        successful_rank_count: bootstrap_receipt.successful_rank_count,
        disposition: bootstrap_disposition_label(bootstrap_receipt.disposition).to_string(),
        refusal: bootstrap_receipt.refusal.clone(),
        receipt_digest: String::new(),
    };
    bootstrap.receipt_digest = bootstrap.stable_digest();
    let validation_hook = DenseRankValidationHookContract {
        hook_id: String::from(DENSE_RANK_RUNTIME_REFERENCE_VALIDATION_HOOK_ID),
        posture: String::from("deferred_after_train_step"),
        detail: String::from(
            "The promoted generic runtime keeps validation explicit as a hook contract instead of pretending the one-step PGOLF consumer completed later post-train validation.",
        ),
    };
    let checkpoint_hook = DenseRankCheckpointHookContract {
        hook_id: String::from(DENSE_RANK_RUNTIME_REFERENCE_CHECKPOINT_HOOK_ID),
        posture: String::from("receipt_only_no_dense_checkpoint_materialized"),
        detail: String::from(
            "The promoted generic runtime keeps checkpointing explicit as a hook contract instead of pretending the one-step PGOLF consumer already materialized a dense distributed checkpoint.",
        ),
    };
    let mut train_step = DenseRankRuntimeTrainStepReceipt {
        schema_version: 1,
        runtime: runtime.clone(),
        run_id: String::from(run_id),
        runtime_bootstrap_receipt_path: bootstrap_receipt_path.display().to_string(),
        runtime_bootstrap_receipt_digest: bootstrap_receipt.receipt_digest.clone(),
        measurements_path: train_step_receipt.measurements_path.clone(),
        distributed_receipt_path: train_step_receipt.distributed_receipt_path.clone(),
        train_step_receipt_path: train_step_receipt_path.display().to_string(),
        mean_train_loss: train_step_receipt.mean_train_loss,
        train_tokens: train_step_receipt.train_tokens,
        observed_step_ms: train_step_receipt.observed_step_ms,
        gradient_sync_ms: train_step_receipt.gradient_sync_ms,
        optimizer_step_ms: train_step_receipt.optimizer_step_ms,
        validation_hook,
        checkpoint_hook,
        receipt_digest: String::new(),
    };
    train_step.receipt_digest = train_step.stable_digest();
    let mut receipt = DenseRankRuntimeExecutionReceipt {
        schema_version: 1,
        runtime,
        run_id: String::from(run_id),
        bootstrap,
        train_step,
        claim_boundary: String::from(
            "This generic dense-rank execution receipt proves the PGOLF CUDA 8xH100 runtime now feeds one provider-neutral dense-rank receipt family in psionic-train. It does not claim mixed-backend dense training, dense checkpoint closure, or later validation closure by itself.",
        ),
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn bootstrap_disposition_label(
    disposition: ParameterGolfDistributed8xH100RuntimeBootstrapDisposition,
) -> &'static str {
    match disposition {
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::Bootstrapped => "bootstrapped",
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::RefusedRuntimePrerequisites => {
            "refused_runtime_prerequisites"
        }
        ParameterGolfDistributed8xH100RuntimeBootstrapDisposition::ChildProcessFailed => {
            "child_process_failed"
        }
    }
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable dense-rank runtime serialization"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        dense_rank_runtime_reference_contract, parameter_golf_dense_rank_runtime_identity,
        DENSE_RANK_RUNTIME_PARAMETER_GOLF_CONSUMER_LANE_ID,
    };

    #[test]
    fn dense_rank_runtime_reference_contract_is_self_digesting() {
        let contract = dense_rank_runtime_reference_contract();
        assert_eq!(contract.contract_digest, contract.stable_digest());
    }

    #[test]
    fn parameter_golf_consumer_identity_uses_lane_specific_id() {
        let runtime = parameter_golf_dense_rank_runtime_identity();
        assert_eq!(
            runtime.consumer_lane_id,
            DENSE_RANK_RUNTIME_PARAMETER_GOLF_CONSUMER_LANE_ID
        );
    }
}
