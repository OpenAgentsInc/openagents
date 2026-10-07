use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_cross_provider_program_run_graph, canonical_dense_rank_recovery_contract,
    canonical_dense_topology_revision_contract, canonical_training_execution_evidence_bundle,
    cross_provider_training_program_manifest, CrossProviderProgramRunGraphError,
    CrossProviderTrainingProgramManifestError, DenseRankRecoveryContractError,
    DenseTopologyRevisionContractError, TrainingExecutionEvidenceBundleError,
};

/// Stable schema version for the first multi-provider dense CUDA proof run.
pub const FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_SCHEMA_VERSION: &str =
    "psionic.first_multi_provider_dense_cuda_run.v1";
/// Stable fixture path for the proof-run bundle.
pub const FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_FIXTURE_PATH: &str =
    "fixtures/training/first_multi_provider_dense_cuda_run_v1.json";
/// Stable checker path for the proof-run bundle.
pub const FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_CHECK_SCRIPT_PATH: &str =
    "scripts/check-first-multi-provider-dense-cuda-run.sh";
/// Stable after-action audit path for the proof run.
pub const FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_AUDIT_PATH: &str =
    "docs/audits/2026-03-25-first-multi-provider-dense-cuda-run-audit.md";

/// Error surfaced while building, validating, or writing the proof-run bundle.
#[derive(Debug, Error)]
pub enum FirstMultiProviderDenseCudaRunError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Serialize(#[from] serde_json::Error),
    #[error(transparent)]
    ProgramManifest(#[from] CrossProviderTrainingProgramManifestError),
    #[error(transparent)]
    WholeProgramRunGraph(#[from] CrossProviderProgramRunGraphError),
    #[error(transparent)]
    DenseRecovery(#[from] DenseRankRecoveryContractError),
    #[error(transparent)]
    DenseTopology(#[from] DenseTopologyRevisionContractError),
    #[error(transparent)]
    ExecutionEvidence(#[from] TrainingExecutionEvidenceBundleError),
    #[error("first multi-provider dense CUDA run bundle is invalid: {detail}")]
    InvalidBundle { detail: String },
}

/// Final disposition for the proof run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstMultiProviderDenseCudaRunDisposition {
    BoundedSuccess,
}

/// One retained artifact in the proof-run bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstMultiProviderDenseCudaRunArtifactRef {
    pub artifact_role: String,
    pub artifact_path: String,
    pub detail: String,
}

/// One phase in the proof run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstMultiProviderDenseCudaRunPhase {
    pub phase_id: String,
    pub topology_revision_id: String,
    pub world_size: u16,
    pub active_provider_ids: Vec<String>,
    pub controlling_contract_id: String,
    pub checkpoint_manifest_digest: String,
    pub detail: String,
}

/// One retained recovery event inside the proof run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstMultiProviderDenseCudaRecoveryEvent {
    pub event_id: String,
    pub dense_recovery_scenario_id: String,
    pub topology_revision_scenario_id: String,
    pub recovered_rank: u16,
    pub detail: String,
}

/// Canonical retained proof-run bundle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FirstMultiProviderDenseCudaRunBundle {
    pub schema_version: String,
    pub run_id: String,
    pub program_manifest_id: String,
    pub program_manifest_digest: String,
    pub whole_program_run_graph_digest: String,
    pub dense_recovery_contract_digest: String,
    pub dense_topology_revision_contract_digest: String,
    pub provider_neutral_execution_bundle_id: String,
    pub provider_neutral_execution_bundle_digest: String,
    pub phases: Vec<FirstMultiProviderDenseCudaRunPhase>,
    pub recovery_events: Vec<FirstMultiProviderDenseCudaRecoveryEvent>,
    pub retained_artifacts: Vec<FirstMultiProviderDenseCudaRunArtifactRef>,
    pub after_action_audit_path: String,
    pub final_disposition: FirstMultiProviderDenseCudaRunDisposition,
    pub claim_boundary: String,
    pub bundle_digest: String,
}

impl FirstMultiProviderDenseCudaRunBundle {
    /// Returns the stable digest over the proof-run payload.
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.bundle_digest.clear();
        stable_digest(b"psionic_first_multi_provider_dense_cuda_run|", &clone)
    }

    /// Validates the proof-run bundle against canonical contracts.
    pub fn validate(&self) -> Result<(), FirstMultiProviderDenseCudaRunError> {
        let manifest = cross_provider_training_program_manifest()?;
        let run_graph = canonical_cross_provider_program_run_graph()?;
        let dense_recovery = canonical_dense_rank_recovery_contract()?;
        let dense_topology = canonical_dense_topology_revision_contract()?;
        let execution_bundle = canonical_training_execution_evidence_bundle()?;

        if self.schema_version != FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_SCHEMA_VERSION {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.program_manifest_id != manifest.program_manifest_id
            || self.program_manifest_digest != manifest.program_manifest_digest
        {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("program manifest binding drifted"),
            });
        }
        if self.whole_program_run_graph_digest != run_graph.contract_digest
            || self.dense_recovery_contract_digest != dense_recovery.contract_digest
            || self.dense_topology_revision_contract_digest != dense_topology.contract_digest
        {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("one or more proof-run contract digests drifted"),
            });
        }
        if self.provider_neutral_execution_bundle_id != execution_bundle.bundle_id
            || self.provider_neutral_execution_bundle_digest != execution_bundle.bundle_digest
        {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("provider-neutral execution bundle binding drifted"),
            });
        }
        if self.phases.len() != 4 {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("expected exactly four proof-run phases"),
            });
        }
        if !self.phases.iter().any(|phase| {
            phase.active_provider_ids == vec![String::from("google"), String::from("runpod")]
        }) {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("proof run must contain one simultaneously mixed Google plus RunPod dense phase"),
            });
        }
        let recovery_event = self
            .recovery_events
            .iter()
            .find(|event| {
                event.dense_recovery_scenario_id
                    == "dense_rank.provider_loss.rank2.cross_provider_replace"
            })
            .ok_or_else(|| FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from(
                    "proof run must retain the provider-loss recovery event over rank 2",
                ),
            })?;
        if recovery_event.topology_revision_scenario_id
            != "dense_topology.replace_rank.rank2_cross_provider"
            || recovery_event.recovered_rank != 2
        {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("provider-loss recovery event drifted from dense recovery or topology-revision truth"),
            });
        }
        if self.after_action_audit_path != FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_AUDIT_PATH {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("after-action audit path drifted"),
            });
        }
        let required_artifacts = [
            "provider_neutral_execution_evidence_bundle",
            "dense_rank_recovery_contract",
            "dense_topology_revision_contract",
            "after_action_audit",
        ];
        for artifact_role in required_artifacts {
            if !self
                .retained_artifacts
                .iter()
                .any(|artifact| artifact.artifact_role == artifact_role)
            {
                return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                    detail: format!("proof run lost required retained artifact `{artifact_role}`"),
                });
            }
        }
        if self.final_disposition != FirstMultiProviderDenseCudaRunDisposition::BoundedSuccess {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("proof run final disposition drifted"),
            });
        }
        if self.bundle_digest != self.stable_digest() {
            return Err(FirstMultiProviderDenseCudaRunError::InvalidBundle {
                detail: String::from("bundle_digest does not match the stable digest"),
            });
        }
        Ok(())
    }
}

/// Returns the canonical retained proof-run bundle.
pub fn canonical_first_multi_provider_dense_cuda_run_bundle(
) -> Result<FirstMultiProviderDenseCudaRunBundle, FirstMultiProviderDenseCudaRunError> {
    let manifest = cross_provider_training_program_manifest()?;
    let run_graph = canonical_cross_provider_program_run_graph()?;
    let dense_recovery = canonical_dense_rank_recovery_contract()?;
    let dense_topology = canonical_dense_topology_revision_contract()?;
    let execution_bundle = canonical_training_execution_evidence_bundle()?;
    let checkpoint_manifest_digest = dense_recovery
        .scenarios
        .iter()
        .find_map(|scenario| scenario.checkpoint_manifest_digest.clone())
        .expect("dense recovery contract must keep checkpoint manifest digest");

    let phases = vec![
        FirstMultiProviderDenseCudaRunPhase {
            phase_id: String::from("bootstrap-runpod-8xh100"),
            topology_revision_id: String::from("psion-xprovider-pretrain-topology-r1"),
            world_size: 8,
            active_provider_ids: vec![String::from("runpod")],
            controlling_contract_id: run_graph.contract_digest.clone(),
            checkpoint_manifest_digest: checkpoint_manifest_digest.clone(),
            detail: String::from(
                "The run bootstrapped as the existing 8-rank RunPod dense CUDA mesh under the shared whole-program run graph.",
            ),
        },
        FirstMultiProviderDenseCudaRunPhase {
            phase_id: String::from("barrier-grow-to-10"),
            topology_revision_id: String::from("psion-xprovider-pretrain-topology-r3"),
            world_size: 10,
            active_provider_ids: vec![String::from("google"), String::from("runpod")],
            controlling_contract_id: dense_topology.contract_digest.clone(),
            checkpoint_manifest_digest: checkpoint_manifest_digest.clone(),
            detail: String::from(
                "The run paused at a checkpoint barrier, grew from 8 to 10 dense ranks, and admitted two Google CUDA ranks through the grow-world checkpoint-barrier topology revision.",
            ),
        },
        FirstMultiProviderDenseCudaRunPhase {
            phase_id: String::from("provider-loss-rank2-recovery"),
            topology_revision_id: String::from("psion-xprovider-pretrain-topology-r2"),
            world_size: 10,
            active_provider_ids: vec![String::from("google"), String::from("runpod")],
            controlling_contract_id: dense_recovery.contract_digest.clone(),
            checkpoint_manifest_digest: checkpoint_manifest_digest.clone(),
            detail: String::from(
                "One RunPod dense rank was replaced by a Google spare through the admitted provider-loss replace-rank recovery path without dropping world size.",
            ),
        },
        FirstMultiProviderDenseCudaRunPhase {
            phase_id: String::from("mixed-provider-steady-state"),
            topology_revision_id: String::from("psion-xprovider-pretrain-topology-r2"),
            world_size: 10,
            active_provider_ids: vec![String::from("google"), String::from("runpod")],
            controlling_contract_id: execution_bundle.bundle_digest.clone(),
            checkpoint_manifest_digest: checkpoint_manifest_digest,
            detail: String::from(
                "The run completed a bounded steady-state mixed-provider CUDA phase after grow-world admission and one provider-loss repair event.",
            ),
        },
    ];

    let recovery_events = vec![FirstMultiProviderDenseCudaRecoveryEvent {
        event_id: String::from("recover-rank2-provider-loss"),
        dense_recovery_scenario_id: String::from(
            "dense_rank.provider_loss.rank2.cross_provider_replace",
        ),
        topology_revision_scenario_id: String::from(
            "dense_topology.replace_rank.rank2_cross_provider",
        ),
        recovered_rank: 2,
        detail: String::from(
            "Rank 2 moved from the departed RunPod provider slot onto a Google spare under the admitted replace-rank recovery and topology-revision contracts.",
        ),
    }];

    let retained_artifacts = vec![
        FirstMultiProviderDenseCudaRunArtifactRef {
            artifact_role: String::from("provider_neutral_execution_evidence_bundle"),
            artifact_path: String::from(
                crate::TRAINING_EXECUTION_EVIDENCE_BUNDLE_FIXTURE_PATH,
            ),
            detail: String::from(
                "Provider-neutral execution evidence still anchors the shared launch, runtime, checkpoint, metric, and validator proof surface.",
            ),
        },
        FirstMultiProviderDenseCudaRunArtifactRef {
            artifact_role: String::from("dense_rank_recovery_contract"),
            artifact_path: String::from(crate::DENSE_RANK_RECOVERY_CONTRACT_FIXTURE_PATH),
            detail: String::from(
                "Dense recovery receipts prove the admitted provider-loss repair path used by the run.",
            ),
        },
        FirstMultiProviderDenseCudaRunArtifactRef {
            artifact_role: String::from("dense_topology_revision_contract"),
            artifact_path: String::from(crate::DENSE_TOPOLOGY_REVISION_CONTRACT_FIXTURE_PATH),
            detail: String::from(
                "Controlled topology revision receipts prove the grow-world and replace-rank transitions used by the run.",
            ),
        },
        FirstMultiProviderDenseCudaRunArtifactRef {
            artifact_role: String::from("after_action_audit"),
            artifact_path: String::from(FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_AUDIT_PATH),
            detail: String::from(
                "The after-action audit states the exact proof surface and exact claim boundary for the first bounded multi-provider dense CUDA run.",
            ),
        },
    ];

    let mut bundle = FirstMultiProviderDenseCudaRunBundle {
        schema_version: String::from(FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_SCHEMA_VERSION),
        run_id: String::from("psion-xprovider-pretrain-multi-provider-cuda-20260325"),
        program_manifest_id: manifest.program_manifest_id.clone(),
        program_manifest_digest: manifest.program_manifest_digest.clone(),
        whole_program_run_graph_digest: run_graph.contract_digest.clone(),
        dense_recovery_contract_digest: dense_recovery.contract_digest.clone(),
        dense_topology_revision_contract_digest: dense_topology.contract_digest.clone(),
        provider_neutral_execution_bundle_id: execution_bundle.bundle_id.clone(),
        provider_neutral_execution_bundle_digest: execution_bundle.bundle_digest.clone(),
        phases,
        recovery_events,
        retained_artifacts,
        after_action_audit_path: String::from(FIRST_MULTI_PROVIDER_DENSE_CUDA_RUN_AUDIT_PATH),
        final_disposition: FirstMultiProviderDenseCudaRunDisposition::BoundedSuccess,
        claim_boundary: String::from(
            "This retained proof run closes one bounded multi-provider dense CUDA program that spans Google and RunPod under the shared contracts. It does not claim mixed-backend training, public swarm compute, or generic production hardening.",
        ),
        bundle_digest: String::new(),
    };
    bundle.bundle_digest = bundle.stable_digest();
    bundle.validate()?;
    Ok(bundle)
}

/// Writes the canonical retained proof-run bundle to disk.
pub fn write_first_multi_provider_dense_cuda_run_bundle(
    output_path: impl AsRef<Path>,
) -> Result<(), FirstMultiProviderDenseCudaRunError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FirstMultiProviderDenseCudaRunError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle = canonical_first_multi_provider_dense_cuda_run_bundle()?;
    let json = serde_json::to_vec_pretty(&bundle)?;
    fs::write(output_path, json).map_err(|error| FirstMultiProviderDenseCudaRunError::Write {
        path: output_path.display().to_string(),
        error,
    })?;
    Ok(())
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(
        serde_json::to_vec(value)
            .expect("first multi-provider dense CUDA run digest serialization must work"),
    );
    hex::encode(hasher.finalize())
}
