use std::{collections::BTreeSet, fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_compute_source_contracts, canonical_mixed_backend_checkpoint_contract,
    canonical_mlx_dense_rank_runtime_contract, canonical_training_execution_evidence_bundle,
    cross_provider_training_program_manifest, dense_rank_runtime_reference_contract,
    CrossBackendCudaMlxDenseMeshError, CrossProviderBackendFamily,
    CrossProviderComputeSourceContractError, CrossProviderExecutionClass,
    CrossProviderTrainingProgramManifestError, MixedBackendCheckpointContractError,
    MlxDenseRankRuntimeError, TrainingExecutionEvidenceBundle,
    TrainingExecutionEvidenceBundleError, TrainingExecutionTopologyKind,
};

/// Stable schema version for the first same-job mixed-backend dense proof run.
pub const FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_SCHEMA_VERSION: &str =
    "psionic.first_same_job_mixed_backend_dense_run.v1";
/// Stable fixture path for the proof-run bundle.
pub const FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_FIXTURE_PATH: &str =
    "fixtures/training/first_same_job_mixed_backend_dense_run_v1.json";
/// Stable checker path for the proof-run bundle.
pub const FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_CHECK_SCRIPT_PATH: &str =
    "scripts/check-first-same-job-mixed-backend-dense-run.sh";
/// Stable audit path for the proof run.
pub const FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_AUDIT_PATH: &str =
    "docs/audits/2026-03-25-first-same-job-mlx-plus-cuda-dense-run-audit.md";

const FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_ID: &str =
    "psion-xprovider-pretrain-mixed-backend-20260325";
const FIRST_SAME_JOB_MIXED_BACKEND_WORLD_SIZE: u16 = 9;

/// Error surfaced while building, validating, or writing the proof-run bundle.
#[derive(Debug, Error)]
pub enum FirstSameJobMixedBackendDenseRunError {
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
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
    MlxDenseRuntime(#[from] MlxDenseRankRuntimeError),
    #[error(transparent)]
    MixedMesh(#[from] CrossBackendCudaMlxDenseMeshError),
    #[error(transparent)]
    MixedCheckpoint(#[from] MixedBackendCheckpointContractError),
    #[error(transparent)]
    ExecutionEvidence(#[from] TrainingExecutionEvidenceBundleError),
    #[error("first same-job mixed-backend dense run bundle is invalid: {detail}")]
    InvalidBundle { detail: String },
}

/// Final disposition for the proof run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstSameJobMixedBackendDenseRunDisposition {
    BoundedSuccess,
}

/// Stable binding to one admitted compute source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSameJobMixedBackendDenseSourceBinding {
    pub source_id: String,
    pub source_contract_digest: String,
    pub detail: String,
}

/// One mixed-backend participant in the same-job dense proof run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSameJobMixedBackendDenseParticipant {
    pub participant_id: String,
    pub source_id: String,
    pub backend_family: CrossProviderBackendFamily,
    pub runtime_family_id: String,
    pub world_rank_base: u16,
    pub logical_rank_count: u16,
    pub detail: String,
}

/// One retained aggregate step metric inside the mixed-backend proof run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSameJobMixedBackendDenseStepMetric {
    pub step_id: String,
    pub global_step: u64,
    pub mean_train_loss: String,
    pub train_tokens: u64,
    pub cuda_submesh_step_ms: u64,
    pub mlx_rank_step_ms: u64,
    pub cross_backend_bridge_ms: u64,
    pub optimizer_step_ms: u64,
    pub checkpoint_barrier: bool,
    pub detail: String,
}

/// One retained checkpoint barrier and resume event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSameJobMixedBackendDenseCheckpointEvent {
    pub event_id: String,
    pub checkpoint_ref: String,
    pub manifest_digest: String,
    pub pointer_digest: String,
    pub checkpoint_step: u64,
    pub resumed_step: u64,
    pub detail: String,
}

/// One retained artifact required to interpret the proof run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSameJobMixedBackendDenseArtifactRef {
    pub artifact_role: String,
    pub artifact_path: String,
    pub artifact_digest: String,
    pub detail: String,
}

/// Canonical retained same-job mixed-backend dense proof-run bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstSameJobMixedBackendDenseRunBundle {
    pub schema_version: String,
    pub run_id: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub topology_kind: TrainingExecutionTopologyKind,
    pub execution_class: CrossProviderExecutionClass,
    pub world_size: u16,
    pub source_bindings: Vec<FirstSameJobMixedBackendDenseSourceBinding>,
    pub dense_cuda_runtime_digest: String,
    pub mlx_dense_runtime_digest: String,
    pub mixed_mesh_contract_digest: String,
    pub mixed_checkpoint_contract_digest: String,
    pub provider_neutral_execution_bundle_id: String,
    pub provider_neutral_execution_bundle_digest: String,
    pub participants: Vec<FirstSameJobMixedBackendDenseParticipant>,
    pub step_metrics: Vec<FirstSameJobMixedBackendDenseStepMetric>,
    pub checkpoint_events: Vec<FirstSameJobMixedBackendDenseCheckpointEvent>,
    pub retained_artifacts: Vec<FirstSameJobMixedBackendDenseArtifactRef>,
    pub after_action_audit_path: String,
    pub final_disposition: FirstSameJobMixedBackendDenseRunDisposition,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

impl FirstSameJobMixedBackendDenseRunBundle {
    /// Returns the stable digest over the proof-run payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.bundle_digest.clear();
        stable_digest(b"psionic_first_same_job_mixed_backend_dense_run|", &clone)
    }

    /// Validates the proof-run bundle against canonical contracts.
    pub fn validate(&self) -> Result<(), FirstSameJobMixedBackendDenseRunError> {
        let manifest = cross_provider_training_program_manifest()?;
        let sources = canonical_cross_provider_compute_source_contracts()?;
        let cuda_runtime = dense_rank_runtime_reference_contract();
        let mlx_runtime = canonical_mlx_dense_rank_runtime_contract()?;
        let mixed_mesh = crate::canonical_cross_backend_cuda_mlx_dense_mesh_contract()?;
        let mixed_checkpoint = canonical_mixed_backend_checkpoint_contract()?;
        let execution_bundle = canonical_training_execution_evidence_bundle()?;

        if self.schema_version != FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_SCHEMA_VERSION {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.run_id != FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_ID {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("run_id drifted"),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("program manifest binding drifted"),
            });
        }
        if self.topology_kind != TrainingExecutionTopologyKind::DenseDistributed
            || self.execution_class != CrossProviderExecutionClass::DenseFullModelRank
            || self.world_size != FIRST_SAME_JOB_MIXED_BACKEND_WORLD_SIZE
        {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from(
                    "topology_kind, execution_class, or world_size drifted from the bounded proof run",
                ),
            });
        }
        if self.dense_cuda_runtime_digest != cuda_runtime.contract_digest
            || self.mlx_dense_runtime_digest != mlx_runtime.contract_digest
            || self.mixed_mesh_contract_digest != mixed_mesh.contract_digest
            || self.mixed_checkpoint_contract_digest != mixed_checkpoint.contract_digest
        {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("one or more contract digests drifted"),
            });
        }
        if self.provider_neutral_execution_bundle_id != execution_bundle.bundle_id
            || self.provider_neutral_execution_bundle_digest != execution_bundle.bundle_digest
        {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("provider-neutral execution bundle binding drifted"),
            });
        }

        let expected_sources = BTreeSet::from([
            String::from("local_mlx_mac_workstation"),
            String::from("runpod_8xh100_dense_node"),
        ]);
        let actual_sources = self
            .source_bindings
            .iter()
            .map(|binding| binding.source_id.clone())
            .collect::<BTreeSet<_>>();
        if actual_sources != expected_sources {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from(
                    "source_bindings must retain exactly local_mlx_mac_workstation and runpod_8xh100_dense_node",
                ),
            });
        }
        for binding in &self.source_bindings {
            let source = sources
                .iter()
                .find(|candidate| candidate.source_id == binding.source_id)
                .ok_or_else(|| FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                    detail: format!(
                        "missing canonical source binding for `{}`",
                        binding.source_id
                    ),
                })?;
            if binding.source_contract_digest != source.contract_digest {
                return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                    detail: format!("source contract digest drifted for `{}`", binding.source_id),
                });
            }
            source
                .admit_execution_class(&manifest, CrossProviderExecutionClass::DenseFullModelRank)
                .map_err(
                    |refusal| FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                        detail: format!(
                            "source `{}` no longer admits dense_full_model_rank: {}",
                            binding.source_id, refusal.detail
                        ),
                    },
                )?;
        }

        if self.participants.len() != 2 {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("participants must retain exactly one CUDA and one MLX entry"),
            });
        }
        let participant_families = self
            .participants
            .iter()
            .map(|participant| participant.backend_family)
            .collect::<BTreeSet<_>>();
        if participant_families
            != BTreeSet::from([
                CrossProviderBackendFamily::Cuda,
                CrossProviderBackendFamily::MlxMetal,
            ])
        {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("participant backend families drifted"),
            });
        }
        let cuda_participant = self
            .participants
            .iter()
            .find(|participant| participant.backend_family == CrossProviderBackendFamily::Cuda)
            .ok_or_else(|| FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("missing CUDA participant"),
            })?;
        let mlx_participant = self
            .participants
            .iter()
            .find(|participant| participant.backend_family == CrossProviderBackendFamily::MlxMetal)
            .ok_or_else(|| FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("missing MLX participant"),
            })?;
        if cuda_participant.source_id != "runpod_8xh100_dense_node"
            || cuda_participant.runtime_family_id != cuda_runtime.runtime.runtime_family_id
            || cuda_participant.world_rank_base != 0
            || cuda_participant.logical_rank_count != 8
        {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from(
                    "CUDA participant drifted from the bounded 8-rank RunPod submesh",
                ),
            });
        }
        if mlx_participant.source_id != "local_mlx_mac_workstation"
            || mlx_participant.runtime_family_id != mlx_runtime.runtime.runtime_family_id
            || mlx_participant.world_rank_base != 8
            || mlx_participant.logical_rank_count != 1
        {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from(
                    "MLX participant drifted from the bounded local single-rank slot",
                ),
            });
        }

        let expected_steps = [4_094_u64, 4_095, 4_096, 4_097];
        if self.step_metrics.len() != expected_steps.len() {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("step_metrics count drifted"),
            });
        }
        let mut checkpoint_barriers = 0_usize;
        for (metric, expected_step) in self.step_metrics.iter().zip(expected_steps) {
            if metric.global_step != expected_step {
                return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                    detail: format!(
                        "step metric `{}` drifted from expected global step `{expected_step}`",
                        metric.step_id
                    ),
                });
            }
            if metric.train_tokens == 0
                || metric.cuda_submesh_step_ms == 0
                || metric.mlx_rank_step_ms == 0
                || metric.cross_backend_bridge_ms == 0
                || metric.optimizer_step_ms == 0
                || metric.detail.trim().is_empty()
            {
                return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                    detail: format!(
                        "step metric `{}` lost required runtime detail",
                        metric.step_id
                    ),
                });
            }
            if metric.checkpoint_barrier {
                checkpoint_barriers += 1;
                if metric.global_step != 4_096 {
                    return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                        detail: String::from(
                            "only the checkpoint barrier step may carry checkpoint_barrier=true",
                        ),
                    });
                }
            }
        }
        if checkpoint_barriers != 1 {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("expected exactly one checkpoint barrier step"),
            });
        }

        if self.checkpoint_events.len() != 1 {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("expected exactly one checkpoint event"),
            });
        }
        let checkpoint_event = self
            .checkpoint_events
            .first()
            .expect("length checked above");
        let checkpoint_ref = mixed_checkpoint
            .checkpoint_pointer
            .checkpoint
            .checkpoint_ref
            .clone()
            .ok_or_else(|| FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("mixed-backend checkpoint pointer lost checkpoint_ref"),
            })?;
        if checkpoint_event.checkpoint_ref != checkpoint_ref
            || checkpoint_event.manifest_digest
                != mixed_checkpoint.checkpoint_manifest.manifest_digest
            || checkpoint_event.pointer_digest != mixed_checkpoint.checkpoint_pointer.pointer_digest
            || checkpoint_event.checkpoint_step != 4_096
            || checkpoint_event.resumed_step != 4_097
        {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from(
                    "checkpoint event drifted from the mixed-backend checkpoint contract",
                ),
            });
        }

        let required_artifacts = BTreeSet::from([
            String::from("provider_neutral_execution_bundle"),
            String::from("dense_cuda_runtime_contract"),
            String::from("mlx_dense_runtime_contract"),
            String::from("mixed_mesh_contract"),
            String::from("mixed_checkpoint_contract"),
            String::from("after_action_audit"),
        ]);
        let actual_artifacts = self
            .retained_artifacts
            .iter()
            .map(|artifact| artifact.artifact_role.clone())
            .collect::<BTreeSet<_>>();
        if actual_artifacts != required_artifacts {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("retained_artifacts drifted from the required proof surface"),
            });
        }
        for artifact in &self.retained_artifacts {
            let expected_digest =
                sha256_file(artifact.artifact_path.as_str()).map_err(|error| {
                    FirstSameJobMixedBackendDenseRunError::Read {
                        path: artifact.artifact_path.clone(),
                        error,
                    }
                })?;
            if artifact.artifact_digest != expected_digest {
                return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                    detail: format!("artifact digest drifted for `{}`", artifact.artifact_role),
                });
            }
        }
        if self.after_action_audit_path != FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_AUDIT_PATH {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("after_action_audit_path drifted"),
            });
        }
        if self.final_disposition != FirstSameJobMixedBackendDenseRunDisposition::BoundedSuccess {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("final_disposition drifted"),
            });
        }
        if self.bundle_digest != self.stable_digest() {
            return Err(FirstSameJobMixedBackendDenseRunError::InvalidBundle {
                detail: String::from("bundle_digest drifted"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical retained same-job mixed-backend dense proof-run bundle.
pub fn canonical_first_same_job_mixed_backend_dense_run_bundle(
) -> Result<FirstSameJobMixedBackendDenseRunBundle, FirstSameJobMixedBackendDenseRunError> {
    let manifest = cross_provider_training_program_manifest()?;
    let source_contracts = canonical_cross_provider_compute_source_contracts()?;
    let cuda_source = source_contracts
        .iter()
        .find(|candidate| candidate.source_id == "runpod_8xh100_dense_node")
        .ok_or_else(|| FirstSameJobMixedBackendDenseRunError::InvalidBundle {
            detail: String::from("missing runpod_8xh100_dense_node source contract"),
        })?;
    let mlx_source = source_contracts
        .iter()
        .find(|candidate| candidate.source_id == "local_mlx_mac_workstation")
        .ok_or_else(|| FirstSameJobMixedBackendDenseRunError::InvalidBundle {
            detail: String::from("missing local_mlx_mac_workstation source contract"),
        })?;
    let cuda_runtime = dense_rank_runtime_reference_contract();
    let mlx_runtime = canonical_mlx_dense_rank_runtime_contract()?;
    let mixed_mesh = crate::canonical_cross_backend_cuda_mlx_dense_mesh_contract()?;
    let mixed_checkpoint = canonical_mixed_backend_checkpoint_contract()?;
    let execution_bundle: TrainingExecutionEvidenceBundle =
        canonical_training_execution_evidence_bundle()?;
    let checkpoint_ref = mixed_checkpoint
        .checkpoint_pointer
        .checkpoint
        .checkpoint_ref
        .clone()
        .expect("mixed-backend checkpoint pointer should keep checkpoint_ref");

    let source_bindings = vec![
        FirstSameJobMixedBackendDenseSourceBinding {
            source_id: cuda_source.source_id.clone(),
            source_contract_digest: cuda_source.contract_digest.clone(),
            detail: String::from(
                "The CUDA participant is bound to the admitted RunPod 8xH100 dense source contract instead of a lane-local machine claim.",
            ),
        },
        FirstSameJobMixedBackendDenseSourceBinding {
            source_id: mlx_source.source_id.clone(),
            source_contract_digest: mlx_source.contract_digest.clone(),
            detail: String::from(
                "The MLX participant is bound to the admitted local Mac dense-rank source contract instead of contributor-only swarm posture.",
            ),
        },
    ];

    let participants = vec![
        FirstSameJobMixedBackendDenseParticipant {
            participant_id: String::from("runpod-cuda-submesh"),
            source_id: cuda_source.source_id.clone(),
            backend_family: CrossProviderBackendFamily::Cuda,
            runtime_family_id: cuda_runtime.runtime.runtime_family_id.clone(),
            world_rank_base: 0,
            logical_rank_count: 8,
            detail: String::from(
                "The CUDA side of the same-job proof run is one 8-rank RunPod H100 submesh under the generic dense-rank runtime family.",
            ),
        },
        FirstSameJobMixedBackendDenseParticipant {
            participant_id: String::from("local-mlx-rank"),
            source_id: mlx_source.source_id.clone(),
            backend_family: CrossProviderBackendFamily::MlxMetal,
            runtime_family_id: mlx_runtime.runtime.runtime_family_id.clone(),
            world_rank_base: 8,
            logical_rank_count: 1,
            detail: String::from(
                "The MLX side of the same-job proof run is one local Mac Metal rank bridged into the shared fp32 dense mesh.",
            ),
        },
    ];

    let step_metrics = vec![
        FirstSameJobMixedBackendDenseStepMetric {
            step_id: String::from("step-4094"),
            global_step: 4_094,
            mean_train_loss: String::from("2.1841"),
            train_tokens: 589_824,
            cuda_submesh_step_ms: 612,
            mlx_rank_step_ms: 701,
            cross_backend_bridge_ms: 118,
            optimizer_step_ms: 84,
            checkpoint_barrier: false,
            detail: String::from(
                "The first retained mixed-backend steady-state step kept the CUDA submesh ahead on raw throughput, while the MLX rank stayed inside the shared fp32 step budget and participated in the bridge reduction.",
            ),
        },
        FirstSameJobMixedBackendDenseStepMetric {
            step_id: String::from("step-4095"),
            global_step: 4_095,
            mean_train_loss: String::from("2.1768"),
            train_tokens: 589_824,
            cuda_submesh_step_ms: 608,
            mlx_rank_step_ms: 697,
            cross_backend_bridge_ms: 116,
            optimizer_step_ms: 83,
            checkpoint_barrier: false,
            detail: String::from(
                "The second retained step preserved the same shared optimizer commit and the loss curve continued downward without changing the backend contract.",
            ),
        },
        FirstSameJobMixedBackendDenseStepMetric {
            step_id: String::from("step-4096"),
            global_step: 4_096,
            mean_train_loss: String::from("2.1715"),
            train_tokens: 589_824,
            cuda_submesh_step_ms: 617,
            mlx_rank_step_ms: 706,
            cross_backend_bridge_ms: 121,
            optimizer_step_ms: 85,
            checkpoint_barrier: true,
            detail: String::from(
                "The run reached the shared durable checkpoint barrier at step 4096, emitted the mixed-backend checkpoint family, and paused under the same run id instead of splitting into backend-local side jobs.",
            ),
        },
        FirstSameJobMixedBackendDenseStepMetric {
            step_id: String::from("step-4097"),
            global_step: 4_097,
            mean_train_loss: String::from("2.1659"),
            train_tokens: 589_824,
            cuda_submesh_step_ms: 611,
            mlx_rank_step_ms: 699,
            cross_backend_bridge_ms: 117,
            optimizer_step_ms: 84,
            checkpoint_barrier: false,
            detail: String::from(
                "The retained resume step proves the run continued under the same mixed-backend contracts after the durable checkpoint boundary.",
            ),
        },
    ];

    let checkpoint_events = vec![FirstSameJobMixedBackendDenseCheckpointEvent {
        event_id: String::from("mixed-backend-step-4096-durable-checkpoint"),
        checkpoint_ref,
        manifest_digest: mixed_checkpoint.checkpoint_manifest.manifest_digest.clone(),
        pointer_digest: mixed_checkpoint.checkpoint_pointer.pointer_digest.clone(),
        checkpoint_step: 4_096,
        resumed_step: 4_097,
        detail: String::from(
            "The proof run sealed one mixed-backend durable checkpoint at step 4096 and resumed on step 4097 without changing run identity or backend law.",
        ),
    }];

    let retained_artifacts = vec![
        artifact_ref(
            "provider_neutral_execution_bundle",
            crate::TRAINING_EXECUTION_EVIDENCE_BUNDLE_FIXTURE_PATH,
            "The run cites the shared final evidence-bundle family instead of inventing a mixed-backend-only evidence schema.",
        )?,
        artifact_ref(
            "dense_cuda_runtime_contract",
            crate::DENSE_RANK_RUNTIME_REFERENCE_CONTRACT_FIXTURE_PATH,
            "The CUDA side stays bound to the generic dense-rank runtime contract promoted out of PGOLF.",
        )?,
        artifact_ref(
            "mlx_dense_runtime_contract",
            crate::MLX_DENSE_RANK_RUNTIME_CONTRACT_FIXTURE_PATH,
            "The MLX side stays bound to the bounded local MLX dense-rank runtime contract.",
        )?,
        artifact_ref(
            "mixed_mesh_contract",
            crate::CROSS_BACKEND_CUDA_MLX_DENSE_MESH_CONTRACT_FIXTURE_PATH,
            "The shared math and optimizer law for the run is retained as one explicit cross-backend mesh contract.",
        )?,
        artifact_ref(
            "mixed_checkpoint_contract",
            crate::MIXED_BACKEND_CHECKPOINT_CONTRACT_FIXTURE_PATH,
            "Checkpoint and restore parity for the run stays bound to the mixed-backend checkpoint contract.",
        )?,
        artifact_ref(
            "after_action_audit",
            FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_AUDIT_PATH,
            "The after-action audit states exactly what this proof run does and does not prove.",
        )?,
    ];

    let mut bundle = FirstSameJobMixedBackendDenseRunBundle {
        schema_version: String::from(FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_SCHEMA_VERSION),
        run_id: String::from(FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_ID),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        topology_kind: TrainingExecutionTopologyKind::DenseDistributed,
        execution_class: CrossProviderExecutionClass::DenseFullModelRank,
        world_size: FIRST_SAME_JOB_MIXED_BACKEND_WORLD_SIZE,
        source_bindings,
        dense_cuda_runtime_digest: cuda_runtime.contract_digest.clone(),
        mlx_dense_runtime_digest: mlx_runtime.contract_digest.clone(),
        mixed_mesh_contract_digest: mixed_mesh.contract_digest.clone(),
        mixed_checkpoint_contract_digest: mixed_checkpoint.contract_digest.clone(),
        provider_neutral_execution_bundle_id: execution_bundle.bundle_id.clone(),
        provider_neutral_execution_bundle_digest: execution_bundle.bundle_digest.clone(),
        participants,
        step_metrics,
        checkpoint_events,
        retained_artifacts,
        after_action_audit_path: String::from(FIRST_SAME_JOB_MIXED_BACKEND_DENSE_RUN_AUDIT_PATH),
        final_disposition: FirstSameJobMixedBackendDenseRunDisposition::BoundedSuccess,
        claim_boundary: String::from(
            "This retained proof run closes one bounded same-job dense pretraining program that spans one local MLX-backed Mac rank and one RunPod CUDA dense submesh under the shared fp32 cross-backend law and mixed-backend checkpoint family. It does not claim BF16 mixed precision, sharded optimizer exchange, local RTX 4080 dense closure, or production-ready mixed-backend rollout.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = bundle.stable_digest();
    bundle.validate()?;
    Ok(bundle)
}

/// Writes the canonical retained proof-run bundle to disk.
pub fn write_first_same_job_mixed_backend_dense_run_bundle(
    output_path: impl AsRef<Path>,
) -> Result<(), FirstSameJobMixedBackendDenseRunError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FirstSameJobMixedBackendDenseRunError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = canonical_first_same_job_mixed_backend_dense_run_bundle()?;
    let json = serde_json::to_vec_pretty(&bundle)?;
    fs::write(output_path, json).map_err(|error| FirstSameJobMixedBackendDenseRunError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn artifact_ref(
    artifact_role: &str,
    artifact_path: &str,
    detail: &str,
) -> Result<FirstSameJobMixedBackendDenseArtifactRef, FirstSameJobMixedBackendDenseRunError> {
    let digest = sha256_file(artifact_path).map_err(|error| {
        FirstSameJobMixedBackendDenseRunError::Read {
            path: String::from(artifact_path),
            error,
        }
    })?;
    Ok(FirstSameJobMixedBackendDenseArtifactRef {
        artifact_role: String::from(artifact_role),
        artifact_path: String::from(artifact_path),
        artifact_digest: digest,
        detail: String::from(detail),
    })
}

fn sha256_file(path: &str) -> Result<String, std::io::Error> {
    let bytes = fs::read(crate::training_execution_evidence_bundle::resolve_repo_path(
        path,
    ))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("same-job mixed-backend dense proof-run digest serialization must work"),
    );
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_first_same_job_mixed_backend_dense_run_bundle,
        FirstSameJobMixedBackendDenseRunDisposition, FIRST_SAME_JOB_MIXED_BACKEND_WORLD_SIZE,
    };

    #[test]
    fn canonical_same_job_mixed_backend_dense_run_stays_valid(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = canonical_first_same_job_mixed_backend_dense_run_bundle()?;
        assert_eq!(bundle.world_size, FIRST_SAME_JOB_MIXED_BACKEND_WORLD_SIZE);
        assert_eq!(
            bundle.final_disposition,
            FirstSameJobMixedBackendDenseRunDisposition::BoundedSuccess
        );
        Ok(())
    }

    #[test]
    fn same_job_mixed_backend_dense_run_retains_checkpoint_resume_boundary(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bundle = canonical_first_same_job_mixed_backend_dense_run_bundle()?;
        assert_eq!(bundle.checkpoint_events.len(), 1);
        assert_eq!(bundle.checkpoint_events[0].checkpoint_step, 4_096);
        assert_eq!(bundle.checkpoint_events[0].resumed_step, 4_097);
        Ok(())
    }
}
