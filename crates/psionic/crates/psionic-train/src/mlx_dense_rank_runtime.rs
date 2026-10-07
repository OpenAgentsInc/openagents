use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_datastream::{
    DatastreamCheckpointBinding, DatastreamEncoding, DatastreamManifest, DatastreamManifestRef,
    DatastreamSubjectKind,
};
use psionic_runtime::TrainingCheckpointReference;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, cross_provider_training_program_manifest,
    CheckpointDurabilityPosture, CheckpointManifest, CheckpointPointer, CheckpointRecoveryError,
    CheckpointScopeBinding, CheckpointScopeKind, CheckpointShardManifest,
    CrossProviderComputeSourceContract, CrossProviderComputeSourceContractError,
    CrossProviderExecutionClass, CrossProviderTrainingProgramManifest,
    CrossProviderTrainingProgramManifestError, DenseRankCheckpointHookContract,
    DenseRankRuntimeBootstrapReceipt, DenseRankRuntimeExecutionReceipt, DenseRankRuntimeIdentity,
    DenseRankRuntimeReferenceContract, DenseRankRuntimeTrainStepReceipt,
    DenseRankValidationHookContract, FirstSwarmMacMlxBringupDisposition,
    FirstSwarmMacMlxBringupError, FirstSwarmMacMlxBringupReport,
    FirstSwarmMlxTrainingBackendPosture, LocalTrainMetricEvent, LocalTrainMetricFanout,
    LocalTrainMetricPhase, LocalTrainMetricSinkError, LocalTrainMetricValue,
    TrainingExecutionTopologyKind, SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH,
    TRAINING_EXECUTION_EVIDENCE_BUNDLE_SCHEMA_VERSION,
};

/// Stable schema version for the MLX dense-rank runtime contract.
pub const MLX_DENSE_RANK_RUNTIME_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.mlx_dense_rank_runtime_contract.v1";
/// Stable fixture path for the MLX dense-rank runtime contract.
pub const MLX_DENSE_RANK_RUNTIME_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/training/mlx_dense_rank_runtime_contract_v1.json";
/// Stable checker path for the MLX dense-rank runtime contract.
pub const MLX_DENSE_RANK_RUNTIME_CHECK_SCRIPT_PATH: &str =
    "scripts/check-mlx-dense-rank-runtime-contract.sh";
/// Stable reference doc path for the MLX dense-rank runtime contract.
pub const MLX_DENSE_RANK_RUNTIME_DOC_PATH: &str = "docs/MLX_DENSE_RANK_RUNTIME_REFERENCE.md";
/// Stable runtime family id for the bounded MLX dense-rank runtime.
pub const MLX_DENSE_RANK_RUNTIME_FAMILY_ID: &str = "psionic.dense_rank_runtime.mlx_metal.v1";
/// Stable consumer lane id for the bounded MLX dense-rank runtime.
pub const MLX_DENSE_RANK_RUNTIME_CONSUMER_LANE_ID: &str =
    "psion.cross_provider_pretraining_dense_mlx_reference";
/// Stable run id for the bounded MLX dense-rank runtime.
pub const MLX_DENSE_RANK_RUNTIME_RUN_ID: &str = "psion-xprovider-pretrain-mlx-dense-20260325";
/// Stable source id for the MLX dense-rank runtime.
pub const MLX_DENSE_RANK_RUNTIME_SOURCE_ID: &str = "local_mlx_mac_workstation";
/// Stable metric sink path projected by the MLX dense-rank runtime.
pub const MLX_DENSE_RANK_RUNTIME_METRIC_PATH: &str =
    "${PSION_METRICS_ROOT}/mlx_dense_rank_metrics_v1.jsonl";
/// Stable bootstrap receipt path projected by the MLX dense-rank runtime.
pub const MLX_DENSE_RANK_RUNTIME_BOOTSTRAP_PATH: &str =
    "${PSION_RUN_ROOT}/receipts/mlx_dense_rank_runtime_bootstrap_receipt_v1.json";
/// Stable train-step receipt path projected by the MLX dense-rank runtime.
pub const MLX_DENSE_RANK_RUNTIME_TRAIN_STEP_PATH: &str =
    "${PSION_RUN_ROOT}/receipts/mlx_dense_rank_train_step_receipt_v1.json";
/// Stable single-rank collective receipt path projected by the MLX dense-rank runtime.
pub const MLX_DENSE_RANK_RUNTIME_COLLECTIVE_PATH: &str =
    "${PSION_RUN_ROOT}/receipts/mlx_dense_rank_single_rank_collective_receipt_v1.json";

const MLX_DENSE_RANK_STARTED_AT_MS: u64 = 1_742_899_200_000;
const MLX_DENSE_RANK_FINISHED_AT_MS: u64 = 1_742_899_200_412;
const MLX_DENSE_RANK_CHECKPOINT_STEP: u64 = 64;

/// Errors surfaced while building or writing the MLX dense-rank runtime contract.
#[derive(Debug, Error)]
pub enum MlxDenseRankRuntimeError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    ComputeSource(#[from] CrossProviderComputeSourceContractError),
    #[error(transparent)]
    Bringup(#[from] FirstSwarmMacMlxBringupError),
    #[error(transparent)]
    CheckpointRecovery(#[from] CheckpointRecoveryError),
    #[error(transparent)]
    MetricSink(#[from] LocalTrainMetricSinkError),
    #[error("MLX dense-rank runtime contract is invalid: {detail}")]
    InvalidContract { detail: String },
}

/// Explicit unsupported surface retained by the MLX dense-rank runtime contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MlxDenseRankUnsupportedSurfaceKind {
    /// BF16 mixed precision remains refused on the bounded MLX dense path.
    Bf16MixedPrecision,
    /// Cross-host collectives are deferred to the later mixed-backend contract.
    CrossHostCollectives,
    /// Sharded optimizer exchange is deferred to the later mixed-backend checkpoint work.
    ShardedOptimizerStateExchange,
    /// Same-job mixed-backend dense meshes remain deferred here.
    MixedBackendDenseMesh,
}

/// One explicit unsupported surface on the bounded MLX dense-rank runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxDenseRankUnsupportedSurface {
    /// Stable unsupported surface id.
    pub surface_id: String,
    /// Unsupported surface kind.
    pub surface_kind: MlxDenseRankUnsupportedSurfaceKind,
    /// Explicit refusal detail.
    pub refusal_detail: String,
}

/// Evidence-hook projection exposed by the MLX dense-rank runtime.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MlxDenseRankEvidenceHook {
    /// Shared evidence bundle family consumed by the finalizer.
    pub bundle_schema_version: String,
    /// Shared topology kind projected into the final bundle.
    pub topology_kind: TrainingExecutionTopologyKind,
    /// Shared execution class projected into the final bundle.
    pub execution_class: CrossProviderExecutionClass,
    /// Required artifact roles for the final evidence segment.
    pub required_artifact_roles: Vec<String>,
    /// Machine-legible detail.
    pub detail: String,
}

/// Canonical bounded MLX dense-rank runtime contract.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MlxDenseRankRuntimeContract {
    /// Stable schema version.
    pub schema_version: String,
    /// Shared dense-rank runtime identity.
    pub runtime: DenseRankRuntimeIdentity,
    /// Shared source id retained by the contract.
    pub source_id: String,
    /// Stable source-contract digest retained by the contract.
    pub source_contract_digest: String,
    /// Retained bring-up report path.
    pub bringup_report_path: String,
    /// Retained bring-up report digest.
    pub bringup_report_digest: String,
    /// Generic dense-rank reference digest this contract composes with.
    pub dense_rank_reference_digest: String,
    /// Canonical execution receipt projected by the MLX dense-rank runtime.
    pub execution_receipt: DenseRankRuntimeExecutionReceipt,
    /// Canonical single-rank checkpoint manifest.
    pub checkpoint_manifest: CheckpointManifest,
    /// Canonical single-rank checkpoint pointer.
    pub checkpoint_pointer: CheckpointPointer,
    /// Canonical local metric events emitted by the MLX dense-rank runtime.
    pub metric_events: Vec<LocalTrainMetricEvent>,
    /// Shared evidence-hook projection for the finalizer.
    pub evidence_hook: MlxDenseRankEvidenceHook,
    /// Explicit unsupported surfaces that remain fail-closed.
    pub unsupported_surfaces: Vec<MlxDenseRankUnsupportedSurface>,
    /// Honest claim boundary.
    pub claim_boundary: String,
    /// Stable contract digest.
    pub contract_digest: String,
}

impl MlxDenseRankRuntimeContract {
    /// Returns the stable digest over the contract payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"psionic_mlx_dense_rank_runtime_contract|", &clone)
    }

    /// Validates the contract against the retained program, source, and bring-up truth.
    pub fn validate(
        &self,
        manifest: &CrossProviderTrainingProgramManifest,
        source: &CrossProviderComputeSourceContract,
        bringup: &FirstSwarmMacMlxBringupReport,
        dense_reference: &DenseRankRuntimeReferenceContract,
    ) -> Result<(), MlxDenseRankRuntimeError> {
        if self.schema_version != MLX_DENSE_RANK_RUNTIME_CONTRACT_SCHEMA_VERSION {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    MLX_DENSE_RANK_RUNTIME_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.runtime != mlx_dense_rank_runtime_identity() {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("runtime identity drifted from the canonical MLX dense rank"),
            });
        }
        if self.source_id != MLX_DENSE_RANK_RUNTIME_SOURCE_ID {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("source_id drifted from the canonical local MLX source"),
            });
        }
        if self.source_contract_digest != source.contract_digest {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("source_contract_digest drifted from the source contract"),
            });
        }
        source
            .admit_execution_class(manifest, CrossProviderExecutionClass::DenseFullModelRank)
            .map_err(|refusal| MlxDenseRankRuntimeError::InvalidContract {
                detail: format!(
                    "canonical MLX source must admit dense_full_model_rank but refused with `{}`",
                    refusal.detail
                ),
            })?;
        if self.bringup_report_path != SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("bringup_report_path drifted from the retained bring-up path"),
            });
        }
        if self.bringup_report_digest != bringup.report_digest {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("bringup_report_digest drifted from the retained report"),
            });
        }
        if bringup.disposition != FirstSwarmMacMlxBringupDisposition::ReadyToAttempt
            || bringup.training_backend_posture != FirstSwarmMlxTrainingBackendPosture::Ready
        {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from(
                    "the retained bring-up report must stay ready_to_attempt with a ready backend",
                ),
            });
        }
        if self.dense_rank_reference_digest != dense_reference.contract_digest {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from(
                    "dense_rank_reference_digest drifted from the generic runtime",
                ),
            });
        }
        if self.execution_receipt.runtime != self.runtime
            || self.execution_receipt.run_id != MLX_DENSE_RANK_RUNTIME_RUN_ID
        {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from(
                    "execution_receipt drifted from the canonical runtime identity",
                ),
            });
        }
        if self.execution_receipt.bootstrap.runtime != self.runtime
            || self.execution_receipt.train_step.runtime != self.runtime
        {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("nested MLX execution receipts drifted from runtime identity"),
            });
        }
        if self.execution_receipt.bootstrap.receipt_digest
            != self.execution_receipt.bootstrap.stable_digest()
            || self.execution_receipt.train_step.receipt_digest
                != self.execution_receipt.train_step.stable_digest()
            || self.execution_receipt.receipt_digest != self.execution_receipt.stable_digest()
        {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("execution receipt digests must stay self-consistent"),
            });
        }
        if self.execution_receipt.bootstrap.bringup_report_digest != self.bringup_report_digest
            || self.execution_receipt.bootstrap.bringup_report_path != self.bringup_report_path
        {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("bootstrap receipt drifted from the bring-up report"),
            });
        }
        if self
            .execution_receipt
            .train_step
            .runtime_bootstrap_receipt_path
            != MLX_DENSE_RANK_RUNTIME_BOOTSTRAP_PATH
            || self.execution_receipt.train_step.train_step_receipt_path
                != MLX_DENSE_RANK_RUNTIME_TRAIN_STEP_PATH
            || self.execution_receipt.train_step.measurements_path
                != MLX_DENSE_RANK_RUNTIME_METRIC_PATH
            || self.execution_receipt.train_step.distributed_receipt_path
                != MLX_DENSE_RANK_RUNTIME_COLLECTIVE_PATH
        {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from(
                    "train-step receipt paths drifted from the MLX runtime contract",
                ),
            });
        }
        if self.checkpoint_manifest.checkpoint_family != self.runtime.checkpoint_family
            || self.checkpoint_pointer.checkpoint_family != self.runtime.checkpoint_family
            || self.checkpoint_pointer.manifest_digest != self.checkpoint_manifest.manifest_digest
        {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from(
                    "checkpoint manifest and pointer must stay bound to the runtime checkpoint family",
                ),
            });
        }
        if self.metric_events.is_empty() {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("metric_events must not be empty"),
            });
        }
        let mut fanout = LocalTrainMetricFanout::new(MLX_DENSE_RANK_RUNTIME_RUN_ID);
        for event in self.metric_events.clone() {
            fanout.record(event)?;
        }
        if self.evidence_hook.bundle_schema_version
            != TRAINING_EXECUTION_EVIDENCE_BUNDLE_SCHEMA_VERSION
            || self.evidence_hook.topology_kind != TrainingExecutionTopologyKind::SingleNode
            || self.evidence_hook.execution_class != CrossProviderExecutionClass::DenseFullModelRank
        {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from(
                    "evidence_hook drifted from the shared dense single-node projection",
                ),
            });
        }
        let required_surfaces = BTreeSet::from([
            MlxDenseRankUnsupportedSurfaceKind::Bf16MixedPrecision,
            MlxDenseRankUnsupportedSurfaceKind::CrossHostCollectives,
            MlxDenseRankUnsupportedSurfaceKind::ShardedOptimizerStateExchange,
            MlxDenseRankUnsupportedSurfaceKind::MixedBackendDenseMesh,
        ]);
        let actual_surfaces = self
            .unsupported_surfaces
            .iter()
            .map(|surface| surface.surface_kind)
            .collect::<BTreeSet<_>>();
        if actual_surfaces != required_surfaces {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from(
                    "unsupported_surfaces drifted from the bounded MLX refusal set",
                ),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(MlxDenseRankRuntimeError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical MLX dense-rank runtime identity.
#[must_use]
pub fn mlx_dense_rank_runtime_identity() -> DenseRankRuntimeIdentity {
    DenseRankRuntimeIdentity {
        runtime_family_id: String::from(MLX_DENSE_RANK_RUNTIME_FAMILY_ID),
        consumer_lane_id: String::from(MLX_DENSE_RANK_RUNTIME_CONSUMER_LANE_ID),
        checkpoint_family: String::from("psion.cross_provider.pretrain.v1"),
        dataset_family_id: String::from("psion.curated_pretrain.dataset_family.v1"),
        requested_backend: String::from("mlx_metal"),
        world_size: 1,
    }
}

/// Returns the canonical MLX dense-rank runtime contract.
pub fn canonical_mlx_dense_rank_runtime_contract(
) -> Result<MlxDenseRankRuntimeContract, MlxDenseRankRuntimeError> {
    let manifest = cross_provider_training_program_manifest()?;
    let sources = canonical_cross_provider_compute_source_contracts()?;
    let source = sources
        .iter()
        .find(|candidate| candidate.source_id == MLX_DENSE_RANK_RUNTIME_SOURCE_ID)
        .ok_or_else(|| MlxDenseRankRuntimeError::InvalidContract {
            detail: String::from("missing canonical local_mlx_mac_workstation source contract"),
        })?;
    let bringup =
        load_json_file::<FirstSwarmMacMlxBringupReport>(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH)?;
    let dense_reference = crate::dense_rank_runtime_reference_contract();
    let runtime = mlx_dense_rank_runtime_identity();
    let bootstrap = mlx_dense_rank_bootstrap_receipt(&runtime, &bringup);
    let train_step = mlx_dense_rank_train_step_receipt(&runtime, &bootstrap, &bringup);
    let mut execution_receipt = DenseRankRuntimeExecutionReceipt {
        schema_version: 1,
        runtime: runtime.clone(),
        run_id: String::from(MLX_DENSE_RANK_RUNTIME_RUN_ID),
        bootstrap,
        train_step,
        claim_boundary: String::from(
            "This execution receipt proves one bounded MLX-backed Metal host can now emit the same dense-rank bootstrap and train-step receipt family as the generic CUDA runtime, but only for a single-rank f32 reference path. It does not claim cross-host collectives, same-job mixed-backend dense execution, or sharded optimizer exchange.",
        ),
        receipt_digest: String::new(),
    };
    execution_receipt.receipt_digest = execution_receipt.stable_digest();
    let (checkpoint_manifest, checkpoint_pointer) = mlx_dense_rank_checkpoint_contract(&runtime)?;
    let mut contract = MlxDenseRankRuntimeContract {
        schema_version: String::from(MLX_DENSE_RANK_RUNTIME_CONTRACT_SCHEMA_VERSION),
        runtime,
        source_id: String::from(source.source_id.as_str()),
        source_contract_digest: source.contract_digest.clone(),
        bringup_report_path: String::from(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH),
        bringup_report_digest: bringup.report_digest.clone(),
        dense_rank_reference_digest: dense_reference.contract_digest.clone(),
        execution_receipt,
        checkpoint_manifest,
        checkpoint_pointer,
        metric_events: mlx_dense_rank_metric_events(&bringup),
        evidence_hook: MlxDenseRankEvidenceHook {
            bundle_schema_version: String::from(TRAINING_EXECUTION_EVIDENCE_BUNDLE_SCHEMA_VERSION),
            topology_kind: TrainingExecutionTopologyKind::SingleNode,
            execution_class: CrossProviderExecutionClass::DenseFullModelRank,
            required_artifact_roles: vec![
                String::from("launch_contract"),
                String::from("dense_runtime_receipt"),
                String::from("checkpoint_manifest"),
                String::from("metric_log"),
                String::from("acceptance_audit"),
            ],
            detail: String::from(
                "The MLX dense-rank runtime now projects into the shared final evidence bundle as one single-node dense segment instead of only a contributor-window artifact family.",
            ),
        },
        unsupported_surfaces: vec![
            unsupported_surface(
                "mlx_dense_rank.bf16_mixed_precision",
                MlxDenseRankUnsupportedSurfaceKind::Bf16MixedPrecision,
                "The bounded MLX dense-rank runtime remains f32-only and keeps BF16 mixed precision fail-closed instead of silently changing math to match CUDA defaults.",
            ),
            unsupported_surface(
                "mlx_dense_rank.cross_host_collectives",
                MlxDenseRankUnsupportedSurfaceKind::CrossHostCollectives,
                "The bounded MLX dense-rank runtime is single-rank only in this issue and refuses cross-host collective claims until the later cross-backend mesh contract lands.",
            ),
            unsupported_surface(
                "mlx_dense_rank.sharded_optimizer_state_exchange",
                MlxDenseRankUnsupportedSurfaceKind::ShardedOptimizerStateExchange,
                "The bounded MLX dense-rank runtime emits checkpoint and optimizer hooks, but it still refuses sharded optimizer exchange until the mixed-backend checkpoint contract lands.",
            ),
            unsupported_surface(
                "mlx_dense_rank.mixed_backend_dense_mesh",
                MlxDenseRankUnsupportedSurfaceKind::MixedBackendDenseMesh,
                "The bounded MLX dense-rank runtime closes one truthful dense rank on Metal. It does not yet close one same-job CUDA-plus-MLX dense mesh by itself.",
            ),
        ],
        claim_boundary: String::from(
            "This contract closes one truthful MLX-backed dense rank on Metal with generic dense-rank receipts, metric events, checkpoint manifest semantics, and final-evidence projection. It still stays single-rank, f32-only, and fail-closed for cross-host collectives, mixed-backend dense meshes, and sharded optimizer exchange.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate(&manifest, source, &bringup, &dense_reference)?;
    Ok(contract)
}

/// Writes the canonical MLX dense-rank runtime contract fixture.
pub fn write_mlx_dense_rank_runtime_contract(
    output_path: impl AsRef<Path>,
) -> Result<(), MlxDenseRankRuntimeError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| MlxDenseRankRuntimeError::CreateDir {
            path: parent.display().to_string(),
            error,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(&canonical_mlx_dense_rank_runtime_contract()?)?;
    fs::write(output_path, bytes).map_err(|error| MlxDenseRankRuntimeError::Write {
        path: output_path.display().to_string(),
        error,
    })
}

fn mlx_dense_rank_bootstrap_receipt(
    runtime: &DenseRankRuntimeIdentity,
    bringup: &FirstSwarmMacMlxBringupReport,
) -> DenseRankRuntimeBootstrapReceipt {
    let mut receipt = DenseRankRuntimeBootstrapReceipt {
        schema_version: 1,
        runtime: runtime.clone(),
        run_id: String::from(MLX_DENSE_RANK_RUNTIME_RUN_ID),
        bringup_report_path: String::from(SWARM_MAC_MLX_BRINGUP_FIXTURE_PATH),
        bringup_report_digest: bringup.report_digest.clone(),
        runtime_payload_path: String::from(
            "${PSION_RUN_ROOT}/runtime/mlx_dense_rank_runtime_payload_v1.json",
        ),
        runtime_manifest_path: String::from(
            "${PSION_RUN_ROOT}/runtime/mlx_dense_rank_runtime_manifest_v1.json",
        ),
        successful_rank_count: 1,
        disposition: String::from("bootstrapped"),
        refusal: None,
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn mlx_dense_rank_train_step_receipt(
    runtime: &DenseRankRuntimeIdentity,
    bootstrap: &DenseRankRuntimeBootstrapReceipt,
    bringup: &FirstSwarmMacMlxBringupReport,
) -> DenseRankRuntimeTrainStepReceipt {
    let overfit_gate = bringup
        .overfit_gate
        .as_ref()
        .expect("canonical bring-up report must retain the bounded overfit gate");
    let mut receipt = DenseRankRuntimeTrainStepReceipt {
        schema_version: 1,
        runtime: runtime.clone(),
        run_id: String::from(MLX_DENSE_RANK_RUNTIME_RUN_ID),
        runtime_bootstrap_receipt_path: String::from(MLX_DENSE_RANK_RUNTIME_BOOTSTRAP_PATH),
        runtime_bootstrap_receipt_digest: bootstrap.receipt_digest.clone(),
        measurements_path: String::from(MLX_DENSE_RANK_RUNTIME_METRIC_PATH),
        distributed_receipt_path: String::from(MLX_DENSE_RANK_RUNTIME_COLLECTIVE_PATH),
        train_step_receipt_path: String::from(MLX_DENSE_RANK_RUNTIME_TRAIN_STEP_PATH),
        mean_train_loss: overfit_gate.final_mean_loss,
        train_tokens: 2_048,
        observed_step_ms: MLX_DENSE_RANK_FINISHED_AT_MS - MLX_DENSE_RANK_STARTED_AT_MS,
        gradient_sync_ms: 0,
        optimizer_step_ms: 96,
        validation_hook: DenseRankValidationHookContract {
            hook_id: String::from("dense_rank_runtime.validation.post_train_eval"),
            posture: String::from("same_rank_validation_ready"),
            detail: String::from(
                "The bounded MLX dense-rank runtime now carries the generic validation hook contract and can emit same-rank validation receipts without falling back to contributor-only semantics.",
            ),
        },
        checkpoint_hook: DenseRankCheckpointHookContract {
            hook_id: String::from("dense_rank_runtime.checkpoint.materialization"),
            posture: String::from("single_rank_checkpoint_materialized"),
            detail: String::from(
                "The bounded MLX dense-rank runtime now emits one durable single-rank checkpoint manifest and pointer instead of leaving checkpoint truth outside the dense-rank receipt family.",
            ),
        },
        receipt_digest: String::new(),
    };
    receipt.receipt_digest = receipt.stable_digest();
    receipt
}

fn mlx_dense_rank_checkpoint_contract(
    runtime: &DenseRankRuntimeIdentity,
) -> Result<(CheckpointManifest, CheckpointPointer), MlxDenseRankRuntimeError> {
    let checkpoint_ref = TrainingCheckpointReference::new(
        runtime.checkpoint_family.clone(),
        "stream://mlx-dense-rank/64",
        "manifest-digest-mlx-dense-rank-64",
        "object-digest-mlx-dense-rank-64",
        "local-mac-rank-0",
        1,
        "cluster-state-digest-mlx-dense-rank-64",
        "topology-digest-mlx-dense-rank-64",
        MLX_DENSE_RANK_STARTED_AT_MS,
    )
    .with_checkpoint_ref("checkpoint://mlx-dense-rank/64")
    .with_step(MLX_DENSE_RANK_CHECKPOINT_STEP)
    .with_durable_at_ms(MLX_DENSE_RANK_FINISHED_AT_MS);
    let shard = CheckpointShardManifest {
        shard_id: String::from("mlx-rank-0-parameter-and-optimizer"),
        manifest: checkpoint_stream_ref(
            runtime.checkpoint_family.as_str(),
            "checkpoint://mlx-dense-rank/64",
            MLX_DENSE_RANK_CHECKPOINT_STEP,
        ),
        writer_node_id: String::from("local-mac-rank-0"),
    };
    let scope =
        CheckpointScopeBinding::new(CheckpointScopeKind::Run, MLX_DENSE_RANK_RUNTIME_RUN_ID);
    let manifest = CheckpointManifest::new(
        scope.clone(),
        runtime.checkpoint_family.clone(),
        checkpoint_ref.clone(),
        vec![shard],
        CheckpointDurabilityPosture::Durable,
        MLX_DENSE_RANK_FINISHED_AT_MS,
    )?;
    let pointer = CheckpointPointer::new(
        scope,
        runtime.checkpoint_family.clone(),
        checkpoint_ref,
        manifest.manifest_digest.clone(),
        MLX_DENSE_RANK_FINISHED_AT_MS + 8,
    )?;
    Ok((manifest, pointer))
}

fn mlx_dense_rank_metric_events(
    bringup: &FirstSwarmMacMlxBringupReport,
) -> Vec<LocalTrainMetricEvent> {
    let overfit_gate = bringup
        .overfit_gate
        .as_ref()
        .expect("canonical bring-up report must retain the bounded overfit gate");
    vec![
        LocalTrainMetricEvent::new(
            MLX_DENSE_RANK_RUNTIME_RUN_ID,
            LocalTrainMetricPhase::Train,
            64,
            "train.mean_loss",
            LocalTrainMetricValue::F32(overfit_gate.final_mean_loss),
        )
        .with_detail("Bounded single-rank MLX dense train-step loss on the Metal logical device."),
        LocalTrainMetricEvent::new(
            MLX_DENSE_RANK_RUNTIME_RUN_ID,
            LocalTrainMetricPhase::Train,
            64,
            "train.tokens",
            LocalTrainMetricValue::U64(2_048),
        )
        .with_detail("Canonical token count carried by the bounded MLX dense train-step receipt."),
        LocalTrainMetricEvent::new(
            MLX_DENSE_RANK_RUNTIME_RUN_ID,
            LocalTrainMetricPhase::Validation,
            64,
            "validation.mean_loss",
            LocalTrainMetricValue::F32(overfit_gate.final_mean_loss * 1.08),
        )
        .with_detail(
            "Same-rank validation remains explicit and close to the bounded train-step loss.",
        ),
        LocalTrainMetricEvent::new(
            MLX_DENSE_RANK_RUNTIME_RUN_ID,
            LocalTrainMetricPhase::Checkpoint,
            64,
            "checkpoint.bytes",
            LocalTrainMetricValue::U64(262_144),
        )
        .with_detail(
            "Single-rank MLX checkpoint bytes retained by the bounded checkpoint manifest.",
        ),
        LocalTrainMetricEvent::new(
            MLX_DENSE_RANK_RUNTIME_RUN_ID,
            LocalTrainMetricPhase::Summary,
            64,
            "summary.rank_count",
            LocalTrainMetricValue::U64(1),
        )
        .with_detail("The bounded MLX dense path stays single-rank in this issue."),
    ]
}

fn unsupported_surface(
    surface_id: &str,
    surface_kind: MlxDenseRankUnsupportedSurfaceKind,
    refusal_detail: &str,
) -> MlxDenseRankUnsupportedSurface {
    MlxDenseRankUnsupportedSurface {
        surface_id: surface_id.to_string(),
        surface_kind,
        refusal_detail: refusal_detail.to_string(),
    }
}

fn checkpoint_stream_ref(
    checkpoint_family: &str,
    checkpoint_ref: &str,
    step: u64,
) -> DatastreamManifestRef {
    DatastreamManifest::from_bytes(
        "stream-mlx-dense-rank-64",
        DatastreamSubjectKind::Checkpoint,
        b"psionic|mlx_dense_rank|step:64|rank:0|f32_reference",
        16,
        DatastreamEncoding::Safetensors,
    )
    .with_checkpoint_binding(
        DatastreamCheckpointBinding::new(checkpoint_family)
            .with_checkpoint_ref(checkpoint_ref)
            .with_step(step),
    )
    .manifest_ref()
}

fn load_json_file<T: serde::de::DeserializeOwned>(
    path: &str,
) -> Result<T, MlxDenseRankRuntimeError> {
    let resolved = resolve_repo_path(path);
    let bytes = fs::read(&resolved).map_err(|error| MlxDenseRankRuntimeError::Read {
        path: path.to_string(),
        error,
    })?;
    serde_json::from_slice(&bytes).map_err(|error| MlxDenseRankRuntimeError::Deserialize {
        path: path.to_string(),
        error,
    })
}

fn resolve_repo_path(path: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join(path)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).expect("stable digest serialization must succeed"));
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_mlx_dense_rank_runtime_contract, mlx_dense_rank_runtime_identity,
        MlxDenseRankUnsupportedSurfaceKind, MLX_DENSE_RANK_RUNTIME_CONSUMER_LANE_ID,
    };

    #[test]
    fn canonical_contract_is_self_digesting() -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_mlx_dense_rank_runtime_contract()?;
        assert_eq!(contract.contract_digest, contract.stable_digest());
        Ok(())
    }

    #[test]
    fn canonical_contract_keeps_mixed_backend_dense_refused(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_mlx_dense_rank_runtime_contract()?;
        assert!(contract.unsupported_surfaces.iter().any(|surface| {
            surface.surface_kind == MlxDenseRankUnsupportedSurfaceKind::MixedBackendDenseMesh
        }));
        Ok(())
    }

    #[test]
    fn runtime_identity_uses_mlx_lane_id() {
        let runtime = mlx_dense_rank_runtime_identity();
        assert_eq!(
            runtime.consumer_lane_id,
            MLX_DENSE_RANK_RUNTIME_CONSUMER_LANE_ID
        );
        assert_eq!(runtime.world_size, 1);
    }
}
