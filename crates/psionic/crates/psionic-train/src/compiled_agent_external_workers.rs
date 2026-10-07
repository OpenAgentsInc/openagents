use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::CompiledAgentEvidenceClass;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_compiled_agent_confidence_policy,
    canonical_compiled_agent_decentralized_role_dry_run_report,
    canonical_compiled_agent_decentralized_role_receipts,
    canonical_compiled_agent_decentralized_roles_contract,
    canonical_compiled_agent_external_contributor_identity,
    canonical_compiled_agent_external_quarantine_report,
    canonical_compiled_agent_external_replay_proposal,
    canonical_compiled_agent_external_runtime_receipt_submission,
    canonical_compiled_agent_external_submission_staging_ledger,
    canonical_compiled_agent_promoted_artifact_contract,
    canonical_compiled_agent_shadow_disagreement_receipts,
    canonical_compiled_agent_stronger_candidate_family_report,
    canonical_compiled_agent_xtrain_cycle_receipt, repo_relative_path,
    retained_compiled_agent_external_benchmark_kit,
    CompiledAgentDecentralizedRoleDefinition, CompiledAgentDecentralizedRoleDryRunReport,
    CompiledAgentDecentralizedRoleKind, CompiledAgentDecentralizedRolesError,
    CompiledAgentExternalBenchmarkError, CompiledAgentExternalContributorIdentity,
    CompiledAgentExternalIntakeError, CompiledAgentExternalQuarantineStatus,
    CompiledAgentExternalReviewState, CompiledAgentExternalValidatorStatus,
    CompiledAgentRoleArtifactRef, CompiledAgentRoleManifest, CompiledAgentRoleReferencePath,
    CompiledAgentRoleReviewBoundary, COMPILED_AGENT_CONFIDENCE_POLICY_FIXTURE_PATH,
    COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_FIXTURE_PATH,
    COMPILED_AGENT_DECENTRALIZED_ROLE_RECEIPTS_FIXTURE_PATH,
    COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_FIXTURE_PATH,
    COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_FIXTURE_PATH,
    COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH,
    COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH,
    COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH,
    COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH,
    COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH,
    COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_FIXTURE_PATH,
    COMPILED_AGENT_SHADOW_DISAGREEMENT_RECEIPTS_FIXTURE_PATH,
    COMPILED_AGENT_STRONGER_CANDIDATE_FAMILY_REPORT_FIXTURE_PATH,
    COMPILED_AGENT_XTRAIN_CYCLE_RECEIPT_FIXTURE_PATH,
};

pub const COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_worker_beta_contract.v1";
pub const COMPILED_AGENT_EXTERNAL_WORKER_RECEIPTS_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_worker_receipts.v1";
pub const COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.external_worker_dry_run.v1";
pub const COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_ID: &str =
    "compiled_agent.external_worker_beta.contract.v1";
pub const COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_ID: &str =
    "compiled_agent.external_worker_beta.dry_run.v1";
pub const COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_worker_beta_contract_v1.json";
pub const COMPILED_AGENT_EXTERNAL_WORKER_RECEIPTS_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_worker_receipts_v1.json";
pub const COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/external/compiled_agent_external_worker_dry_run_v1.json";
pub const COMPILED_AGENT_EXTERNAL_WORKERS_DOC_PATH: &str =
    "docs/COMPILED_AGENT_EXTERNAL_WORKERS.md";
pub const COMPILED_AGENT_EXTERNAL_WORKERS_BIN_PATH: &str =
    "crates/psionic-train/src/bin/compiled_agent_external_workers.rs";

#[derive(Debug, Error)]
pub enum CompiledAgentExternalWorkersError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid external worker contract: {detail}")]
    InvalidContract { detail: String },
    #[error("invalid external worker receipts: {detail}")]
    InvalidReceipts { detail: String },
    #[error("invalid external worker dry run: {detail}")]
    InvalidDryRun { detail: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(transparent)]
    DecentralizedRoles(#[from] CompiledAgentDecentralizedRolesError),
    #[error(transparent)]
    ArtifactContract(#[from] crate::CompiledAgentArtifactContractError),
    #[error(transparent)]
    ExternalBenchmark(#[from] CompiledAgentExternalBenchmarkError),
    #[error(transparent)]
    ExternalIntake(#[from] CompiledAgentExternalIntakeError),
    #[error(transparent)]
    ShadowGovernance(#[from] crate::CompiledAgentShadowGovernanceError),
    #[error(transparent)]
    Xtrain(#[from] crate::CompiledAgentXtrainError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalWorkersAuthorityPaths {
    pub contract_fixture_path: String,
    pub receipts_fixture_path: String,
    pub dry_run_fixture_path: String,
    pub bin_path: String,
    pub doc_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalWorkerRoleDefinition {
    pub role: CompiledAgentDecentralizedRoleKind,
    pub worker_role_id: String,
    pub delegated_role_id: String,
    pub purpose: String,
    pub input_manifest: CompiledAgentRoleManifest,
    pub output_manifest: CompiledAgentRoleManifest,
    pub local_reference_path: CompiledAgentRoleReferencePath,
    pub review_boundary: CompiledAgentRoleReviewBoundary,
    pub validator_gate: String,
    pub claim_boundary: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalWorkerBetaContract {
    pub schema_version: String,
    pub contract_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub contributor_contract_version: String,
    pub source_artifacts: Vec<CompiledAgentRoleArtifactRef>,
    pub roles: Vec<CompiledAgentExternalWorkerRoleDefinition>,
    pub authority_paths: CompiledAgentExternalWorkersAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalWorkerSubmissionReceipt {
    pub role: CompiledAgentDecentralizedRoleKind,
    pub submission_id: String,
    pub contributor: CompiledAgentExternalContributorIdentity,
    pub contract_digest: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub validator_status: CompiledAgentExternalValidatorStatus,
    pub quarantine_status: CompiledAgentExternalQuarantineStatus,
    pub review_state: CompiledAgentExternalReviewState,
    pub promotion_authority_granted: bool,
    pub runtime_authority_granted: bool,
    pub input_refs: Vec<String>,
    pub output_refs: Vec<String>,
    pub source_submission_ids: Vec<String>,
    pub source_receipt_ids: Vec<String>,
    pub emitted_ids: Vec<String>,
    pub failure_classes: Vec<String>,
    pub next_consumer: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalWorkerReceipts {
    pub schema_version: String,
    pub contract_digest: String,
    pub receipts: Vec<CompiledAgentExternalWorkerSubmissionReceipt>,
    pub summary: String,
    pub receipts_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentExternalWorkerDryRunReport {
    pub schema_version: String,
    pub dry_run_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub contract_digest: String,
    pub receipts_digest: String,
    pub benchmark_contract_digest: String,
    pub staging_ledger_digest: String,
    pub quarantine_report_digest: String,
    pub internal_decentralized_dry_run_digest: String,
    pub promoted_contract_digest: String,
    pub xtrain_receipt_digest: String,
    pub stronger_candidate_family_report_digest: String,
    pub confidence_policy_digest: String,
    pub shadow_disagreement_receipts_digest: String,
    pub accepted_submission_count: u32,
    pub rejected_submission_count: u32,
    pub review_required_submission_count: u32,
    pub validator_discipline_unchanged: bool,
    pub rollback_discipline_unchanged: bool,
    pub role_runs: Vec<CompiledAgentExternalWorkerSubmissionReceipt>,
    pub summary: String,
    pub report_digest: String,
}

impl CompiledAgentExternalWorkerBetaContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"compiled_agent_external_worker_beta_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), CompiledAgentExternalWorkersError> {
        if self.schema_version != COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_SCHEMA_VERSION {
            return Err(CompiledAgentExternalWorkersError::InvalidContract {
                detail: String::from("schema_version drifted"),
            });
        }
        if self.contract_id != COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_ID {
            return Err(CompiledAgentExternalWorkersError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.authority_paths.contract_fixture_path
            != COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_FIXTURE_PATH
            || self.authority_paths.receipts_fixture_path
                != COMPILED_AGENT_EXTERNAL_WORKER_RECEIPTS_FIXTURE_PATH
            || self.authority_paths.dry_run_fixture_path
                != COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_FIXTURE_PATH
            || self.authority_paths.bin_path != COMPILED_AGENT_EXTERNAL_WORKERS_BIN_PATH
            || self.authority_paths.doc_path != COMPILED_AGENT_EXTERNAL_WORKERS_DOC_PATH
        {
            return Err(CompiledAgentExternalWorkersError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.roles.len() != 4 {
            return Err(CompiledAgentExternalWorkersError::InvalidContract {
                detail: String::from("expected exactly four external worker roles"),
            });
        }
        let role_ids = self
            .roles
            .iter()
            .map(|role| role.worker_role_id.as_str())
            .collect::<BTreeSet<_>>();
        if role_ids.len() != self.roles.len() {
            return Err(CompiledAgentExternalWorkersError::InvalidContract {
                detail: String::from("worker role ids must stay unique"),
            });
        }
        let expected_refs = canonical_external_worker_source_artifacts()?;
        if self.source_artifacts != expected_refs {
            return Err(CompiledAgentExternalWorkersError::InvalidContract {
                detail: String::from("source artifact set drifted"),
            });
        }
        if self.contract_digest != self.stable_digest() {
            return Err(CompiledAgentExternalWorkersError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentExternalWorkerReceipts {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipts_digest.clear();
        stable_digest(b"compiled_agent_external_worker_receipts|", &clone)
    }

    pub fn validate(
        &self,
        contract: &CompiledAgentExternalWorkerBetaContract,
    ) -> Result<(), CompiledAgentExternalWorkersError> {
        if self.schema_version != COMPILED_AGENT_EXTERNAL_WORKER_RECEIPTS_SCHEMA_VERSION {
            return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                detail: String::from("schema_version drifted"),
            });
        }
        if self.contract_digest != contract.contract_digest {
            return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                detail: String::from("receipts lost contract linkage"),
            });
        }
        if self.receipts.len() != contract.roles.len() {
            return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                detail: String::from("expected one worker receipt per role"),
            });
        }

        let mut accepted = 0;
        let mut rejected = 0;
        let mut review_required = 0;
        for receipt in &self.receipts {
            if receipt.contract_digest != contract.contract_digest {
                return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                    detail: format!("receipt `{}` lost contract digest linkage", receipt.submission_id),
                });
            }
            if receipt.evidence_class != contract.evidence_class {
                return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                    detail: format!("receipt `{}` drifted evidence class", receipt.submission_id),
                });
            }
            if receipt.contributor.contract_version_accepted != contract.contributor_contract_version
            {
                return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                    detail: format!(
                        "receipt `{}` lost contributor contract linkage",
                        receipt.submission_id
                    ),
                });
            }
            if receipt.promotion_authority_granted || receipt.runtime_authority_granted {
                return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                    detail: format!(
                        "receipt `{}` incorrectly granted authority",
                        receipt.submission_id
                    ),
                });
            }
            if receipt.next_consumer == "compiled_agent_runtime_authority" {
                return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                    detail: format!(
                        "receipt `{}` cannot hand external output directly to runtime authority",
                        receipt.submission_id
                    ),
                });
            }
            match receipt.review_state {
                CompiledAgentExternalReviewState::Accepted => {
                    accepted += 1;
                    if receipt.validator_status != CompiledAgentExternalValidatorStatus::Passed {
                        return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                            detail: format!(
                                "accepted receipt `{}` must have passed validator status",
                                receipt.submission_id
                            ),
                        });
                    }
                }
                CompiledAgentExternalReviewState::Rejected => {
                    rejected += 1;
                    if receipt.quarantine_status != CompiledAgentExternalQuarantineStatus::Rejected {
                        return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                            detail: format!(
                                "rejected receipt `{}` must stay rejected in quarantine",
                                receipt.submission_id
                            ),
                        });
                    }
                }
                CompiledAgentExternalReviewState::ReviewRequired => {
                    review_required += 1;
                }
            }
        }
        if accepted == 0 || rejected == 0 || review_required == 0 {
            return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                detail: String::from(
                    "external worker receipts must keep at least one accepted, rejected, and review-required outcome",
                ),
            });
        }
        if self.receipts_digest != self.stable_digest() {
            return Err(CompiledAgentExternalWorkersError::InvalidReceipts {
                detail: String::from("receipts digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentExternalWorkerDryRunReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"compiled_agent_external_worker_dry_run|", &clone)
    }

    pub fn validate(
        &self,
        contract: &CompiledAgentExternalWorkerBetaContract,
        receipts: &CompiledAgentExternalWorkerReceipts,
        base_dry_run: &CompiledAgentDecentralizedRoleDryRunReport,
    ) -> Result<(), CompiledAgentExternalWorkersError> {
        if self.schema_version != COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_SCHEMA_VERSION {
            return Err(CompiledAgentExternalWorkersError::InvalidDryRun {
                detail: String::from("schema_version drifted"),
            });
        }
        if self.dry_run_id != COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_ID {
            return Err(CompiledAgentExternalWorkersError::InvalidDryRun {
                detail: String::from("dry_run_id drifted"),
            });
        }
        if self.contract_digest != contract.contract_digest
            || self.receipts_digest != receipts.receipts_digest
        {
            return Err(CompiledAgentExternalWorkersError::InvalidDryRun {
                detail: String::from("dry run lost contract or receipts linkage"),
            });
        }
        if self.evidence_class != contract.evidence_class {
            return Err(CompiledAgentExternalWorkersError::InvalidDryRun {
                detail: String::from("dry run evidence class drifted"),
            });
        }
        if self.internal_decentralized_dry_run_digest != base_dry_run.report_digest {
            return Err(CompiledAgentExternalWorkersError::InvalidDryRun {
                detail: String::from("base internal dry run linkage drifted"),
            });
        }
        if self.role_runs != receipts.receipts {
            return Err(CompiledAgentExternalWorkersError::InvalidDryRun {
                detail: String::from("dry run role receipts drifted from canonical receipts"),
            });
        }
        if self.report_digest != self.stable_digest() {
            return Err(CompiledAgentExternalWorkersError::InvalidDryRun {
                detail: String::from("dry run digest drifted"),
            });
        }
        Ok(())
    }
}

#[must_use]
pub fn compiled_agent_external_worker_beta_contract_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_external_worker_receipts_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_WORKER_RECEIPTS_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_external_worker_dry_run_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_FIXTURE_PATH)
}

pub fn canonical_compiled_agent_external_worker_beta_contract(
) -> Result<CompiledAgentExternalWorkerBetaContract, CompiledAgentExternalWorkersError> {
    let benchmark_kit = retained_compiled_agent_external_benchmark_kit()?;
    let internal_contract = canonical_compiled_agent_decentralized_roles_contract()?;
    let source_artifacts = canonical_external_worker_source_artifacts()?;
    let roles = canonical_external_worker_roles(&internal_contract, &source_artifacts)?;

    let mut contract = CompiledAgentExternalWorkerBetaContract {
        schema_version: String::from(COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_ID),
        evidence_class: benchmark_kit.evidence_class,
        contributor_contract_version: benchmark_kit.contributor_contract_version,
        source_artifacts,
        roles,
        authority_paths: CompiledAgentExternalWorkersAuthorityPaths {
            contract_fixture_path: String::from(
                COMPILED_AGENT_EXTERNAL_WORKER_BETA_CONTRACT_FIXTURE_PATH,
            ),
            receipts_fixture_path: String::from(COMPILED_AGENT_EXTERNAL_WORKER_RECEIPTS_FIXTURE_PATH),
            dry_run_fixture_path: String::from(COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_FIXTURE_PATH),
            bin_path: String::from(COMPILED_AGENT_EXTERNAL_WORKERS_BIN_PATH),
            doc_path: String::from(COMPILED_AGENT_EXTERNAL_WORKERS_DOC_PATH),
        },
        claim_boundary: String::from(
            "This is a governed external worker beta for the admitted compiled-agent family only. Outside workers can execute bounded replay-generation, ranking, validator-scoring, and bounded-training jobs, but they do not get promotion authority, live runtime authority, task-family widening authority, or validator bypass.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn canonical_compiled_agent_external_worker_receipts(
) -> Result<CompiledAgentExternalWorkerReceipts, CompiledAgentExternalWorkersError> {
    let contract = canonical_compiled_agent_external_worker_beta_contract()?;
    let contributor = canonical_compiled_agent_external_contributor_identity();
    let staging_ledger = canonical_compiled_agent_external_submission_staging_ledger()?;
    let quarantine_report = canonical_compiled_agent_external_quarantine_report()?;
    let replay_proposal = canonical_compiled_agent_external_replay_proposal()?;
    let runtime_submission = canonical_compiled_agent_external_runtime_receipt_submission()?;
    let internal_receipts = canonical_compiled_agent_decentralized_role_receipts()?;

    let review_receipt_ids = review_required_receipt_ids(&staging_ledger);
    let replay_candidate_receipt_ids = quarantine_report.replay_candidate_receipt_ids.clone();
    let accepted_submission_ids = quarantine_report.accepted_submission_ids.clone();
    let review_submission_ids = quarantine_report.review_required_submission_ids.clone();

    let validator_internal = internal_role_receipt(
        &internal_receipts,
        CompiledAgentDecentralizedRoleKind::ValidatorScoring,
    );
    let training_internal = internal_role_receipt(
        &internal_receipts,
        CompiledAgentDecentralizedRoleKind::BoundedModuleTraining,
    );

    let mut receipts = vec![
        CompiledAgentExternalWorkerSubmissionReceipt {
            role: CompiledAgentDecentralizedRoleKind::ReplayGeneration,
            submission_id: String::from(
                "compiled_agent.external_worker_submission.replay_generation.v1",
            ),
            contributor: contributor.clone(),
            contract_digest: contract.contract_digest.clone(),
            evidence_class: contract.evidence_class,
            validator_status: CompiledAgentExternalValidatorStatus::Passed,
            quarantine_status: CompiledAgentExternalQuarantineStatus::ReplayCandidateEligible,
            review_state: CompiledAgentExternalReviewState::Accepted,
            promotion_authority_granted: false,
            runtime_authority_granted: false,
            input_refs: vec![
                String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH),
                String::from(COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH),
                String::from(COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH),
            ],
            output_refs: vec![String::from(COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH)],
            source_submission_ids: accepted_submission_ids,
            source_receipt_ids: replay_candidate_receipt_ids,
            emitted_ids: replay_proposal
                .proposed_samples
                .iter()
                .map(|sample| sample.sample_id.clone())
                .chain(std::iter::once(replay_proposal.proposal_id.clone()))
                .collect(),
            failure_classes: vec![String::from("none")],
            next_consumer: training_internal.next_consumer.clone(),
            detail: String::from(
                "An external replay-generation worker can now produce a bounded replay proposal from admitted external evidence, and that proposal can flow into the same bounded training queue without granting runtime or promotion authority.",
            ),
        },
        CompiledAgentExternalWorkerSubmissionReceipt {
            role: CompiledAgentDecentralizedRoleKind::RankingLabeling,
            submission_id: String::from(
                "compiled_agent.external_worker_submission.ranking_labeling.v1",
            ),
            contributor: contributor.clone(),
            contract_digest: contract.contract_digest.clone(),
            evidence_class: contract.evidence_class,
            validator_status: CompiledAgentExternalValidatorStatus::Passed,
            quarantine_status: CompiledAgentExternalQuarantineStatus::Held,
            review_state: CompiledAgentExternalReviewState::ReviewRequired,
            promotion_authority_granted: false,
            runtime_authority_granted: false,
            input_refs: vec![
                String::from(COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH),
                String::from(COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH),
                String::from(COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH),
            ],
            output_refs: vec![
                String::from(COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH),
                String::from(COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH),
            ],
            source_submission_ids: review_submission_ids,
            source_receipt_ids: review_receipt_ids,
            emitted_ids: vec![
                String::from("compiled_agent.external_worker.label_review.negated_wallet.v1"),
                String::from("compiled_agent.external_worker.label_review.runtime_shadow.v1"),
            ],
            failure_classes: vec![String::from("human_review_required")],
            next_consumer: String::from("compiled_agent.external_worker.review.label_curation"),
            detail: String::from(
                "An external ranking-and-labeling worker can curate review-required external rows, but those outputs stay in the same human-review boundary before they can change replay or held-out truth.",
            ),
        },
        CompiledAgentExternalWorkerSubmissionReceipt {
            role: CompiledAgentDecentralizedRoleKind::ValidatorScoring,
            submission_id: String::from(
                "compiled_agent.external_worker_submission.validator_scoring.v1",
            ),
            contributor: contributor.clone(),
            contract_digest: contract.contract_digest.clone(),
            evidence_class: contract.evidence_class,
            validator_status: CompiledAgentExternalValidatorStatus::Failed,
            quarantine_status: CompiledAgentExternalQuarantineStatus::Rejected,
            review_state: CompiledAgentExternalReviewState::Rejected,
            promotion_authority_granted: false,
            runtime_authority_granted: false,
            input_refs: vec![
                String::from(COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH),
                String::from(COMPILED_AGENT_CONFIDENCE_POLICY_FIXTURE_PATH),
                String::from(COMPILED_AGENT_SHADOW_DISAGREEMENT_RECEIPTS_FIXTURE_PATH),
                String::from(COMPILED_AGENT_STRONGER_CANDIDATE_FAMILY_REPORT_FIXTURE_PATH),
                String::from(COMPILED_AGENT_XTRAIN_CYCLE_RECEIPT_FIXTURE_PATH),
                String::from(COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_FIXTURE_PATH),
            ],
            output_refs: validator_internal.output_refs.clone(),
            source_submission_ids: vec![
                runtime_submission.submission_id.clone(),
                replay_proposal.proposal_id.clone(),
            ],
            source_receipt_ids: vec![replay_proposal.source_receipt_id.clone()],
            emitted_ids: vec![String::from(
                "compiled_agent.external_worker.validator_scoring.review_packet.v1",
            )],
            failure_classes: vec![String::from(
                "promotion_authority_escalation_blocked",
            )],
            next_consumer: String::from("compiled_agent.external_worker.rejection.validator_mismatch"),
            detail: String::from(
                "The retained external validator-scoring submission is rejected because outside workers can score bounded candidates, but they cannot directly move promotion or runtime authority.",
            ),
        },
        CompiledAgentExternalWorkerSubmissionReceipt {
            role: CompiledAgentDecentralizedRoleKind::BoundedModuleTraining,
            submission_id: String::from(
                "compiled_agent.external_worker_submission.bounded_module_training.v1",
            ),
            contributor,
            contract_digest: contract.contract_digest.clone(),
            evidence_class: contract.evidence_class,
            validator_status: CompiledAgentExternalValidatorStatus::Passed,
            quarantine_status: CompiledAgentExternalQuarantineStatus::Held,
            review_state: CompiledAgentExternalReviewState::Accepted,
            promotion_authority_granted: false,
            runtime_authority_granted: false,
            input_refs: vec![
                String::from("fixtures/compiled_agent/compiled_agent_default_row_v1.json"),
                String::from("fixtures/compiled_agent/compiled_agent_learning_receipts_v1.json"),
                String::from("fixtures/compiled_agent/compiled_agent_replay_bundle_v1.json"),
                String::from(COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH),
                String::from(COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH),
            ],
            output_refs: training_internal.output_refs.clone(),
            source_submission_ids: vec![replay_proposal.proposal_id],
            source_receipt_ids: vec![replay_proposal.source_receipt_id],
            emitted_ids: training_internal.emitted_ids.clone(),
            failure_classes: vec![String::from("none")],
            next_consumer: training_internal.next_consumer.clone(),
            detail: String::from(
                "An external bounded-module-training worker can emit candidate artifacts behind the same narrow route and grounded-answer contracts, but those artifacts still queue for validator scoring before any promotion decision.",
            ),
        },
    ];
    receipts.sort_by(|left, right| left.submission_id.cmp(&right.submission_id));

    let mut bundle = CompiledAgentExternalWorkerReceipts {
        schema_version: String::from(COMPILED_AGENT_EXTERNAL_WORKER_RECEIPTS_SCHEMA_VERSION),
        contract_digest: contract.contract_digest.clone(),
        receipts,
        summary: String::new(),
        receipts_digest: String::new(),
    };
    let accepted = bundle
        .receipts
        .iter()
        .filter(|receipt| receipt.review_state == CompiledAgentExternalReviewState::Accepted)
        .count();
    let rejected = bundle
        .receipts
        .iter()
        .filter(|receipt| receipt.review_state == CompiledAgentExternalReviewState::Rejected)
        .count();
    let review_required = bundle
        .receipts
        .iter()
        .filter(|receipt| receipt.review_state == CompiledAgentExternalReviewState::ReviewRequired)
        .count();
    bundle.summary = format!(
        "Compiled-agent external worker receipts retain {} governed outside-worker role submissions with {} accepted, {} rejected, and {} review-required outcomes on the admitted family.",
        bundle.receipts.len(),
        accepted,
        rejected,
        review_required
    );
    bundle.receipts_digest = bundle.stable_digest();
    bundle.validate(&contract)?;
    Ok(bundle)
}

pub fn canonical_compiled_agent_external_worker_dry_run_report(
) -> Result<CompiledAgentExternalWorkerDryRunReport, CompiledAgentExternalWorkersError> {
    let contract = canonical_compiled_agent_external_worker_beta_contract()?;
    let receipts = canonical_compiled_agent_external_worker_receipts()?;
    let benchmark_kit = retained_compiled_agent_external_benchmark_kit()?;
    let staging_ledger = canonical_compiled_agent_external_submission_staging_ledger()?;
    let quarantine_report = canonical_compiled_agent_external_quarantine_report()?;
    let base_dry_run = canonical_compiled_agent_decentralized_role_dry_run_report()?;
    let promoted_contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let xtrain_receipt = canonical_compiled_agent_xtrain_cycle_receipt()?;
    let stronger_family_report = canonical_compiled_agent_stronger_candidate_family_report()?;
    let confidence_policy = canonical_compiled_agent_confidence_policy()?;
    let shadow_disagreements = canonical_compiled_agent_shadow_disagreement_receipts()?;

    let accepted_submission_count = receipts
        .receipts
        .iter()
        .filter(|receipt| receipt.review_state == CompiledAgentExternalReviewState::Accepted)
        .count() as u32;
    let rejected_submission_count = receipts
        .receipts
        .iter()
        .filter(|receipt| receipt.review_state == CompiledAgentExternalReviewState::Rejected)
        .count() as u32;
    let review_required_submission_count = receipts
        .receipts
        .iter()
        .filter(|receipt| receipt.review_state == CompiledAgentExternalReviewState::ReviewRequired)
        .count() as u32;

    let mut report = CompiledAgentExternalWorkerDryRunReport {
        schema_version: String::from(COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_SCHEMA_VERSION),
        dry_run_id: String::from(COMPILED_AGENT_EXTERNAL_WORKER_DRY_RUN_ID),
        evidence_class: contract.evidence_class,
        contract_digest: contract.contract_digest.clone(),
        receipts_digest: receipts.receipts_digest.clone(),
        benchmark_contract_digest: benchmark_kit.contract_digest,
        staging_ledger_digest: staging_ledger.ledger_digest,
        quarantine_report_digest: quarantine_report.report_digest,
        internal_decentralized_dry_run_digest: base_dry_run.report_digest.clone(),
        promoted_contract_digest: promoted_contract.contract_digest,
        xtrain_receipt_digest: xtrain_receipt.receipt_digest,
        stronger_candidate_family_report_digest: stronger_family_report.report_digest,
        confidence_policy_digest: confidence_policy.policy_digest,
        shadow_disagreement_receipts_digest: shadow_disagreements.receipts_digest,
        accepted_submission_count,
        rejected_submission_count,
        review_required_submission_count,
        validator_discipline_unchanged: base_dry_run.validator_discipline_unchanged,
        rollback_discipline_unchanged: base_dry_run.rollback_discipline_unchanged,
        role_runs: receipts.receipts.clone(),
        summary: String::from(
            "External governed worker-role execution now reruns the same narrow replay, labeling, validator-scoring, and bounded-training contracts through accepted, rejected, and review-required outcomes without granting outside promotion or runtime authority.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate(&contract, &receipts, &base_dry_run)?;
    Ok(report)
}

pub fn write_compiled_agent_external_worker_beta_contract(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalWorkerBetaContract, CompiledAgentExternalWorkersError> {
    write_json(
        output_path,
        &canonical_compiled_agent_external_worker_beta_contract()?,
    )
}

pub fn write_compiled_agent_external_worker_receipts(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalWorkerReceipts, CompiledAgentExternalWorkersError> {
    write_json(output_path, &canonical_compiled_agent_external_worker_receipts()?)
}

pub fn write_compiled_agent_external_worker_dry_run(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentExternalWorkerDryRunReport, CompiledAgentExternalWorkersError> {
    write_json(output_path, &canonical_compiled_agent_external_worker_dry_run_report()?)
}

pub fn verify_compiled_agent_external_worker_fixtures(
) -> Result<(), CompiledAgentExternalWorkersError> {
    verify_fixture(
        compiled_agent_external_worker_beta_contract_fixture_path(),
        canonical_compiled_agent_external_worker_beta_contract()?,
    )?;
    verify_fixture(
        compiled_agent_external_worker_receipts_fixture_path(),
        canonical_compiled_agent_external_worker_receipts()?,
    )?;
    verify_fixture(
        compiled_agent_external_worker_dry_run_fixture_path(),
        canonical_compiled_agent_external_worker_dry_run_report()?,
    )?;
    Ok(())
}

pub fn compiled_agent_external_worker_snapshot(
    role: CompiledAgentDecentralizedRoleKind,
) -> Result<
    (
        CompiledAgentExternalWorkerRoleDefinition,
        CompiledAgentExternalWorkerSubmissionReceipt,
    ),
    CompiledAgentExternalWorkersError,
> {
    let contract = canonical_compiled_agent_external_worker_beta_contract()?;
    let receipts = canonical_compiled_agent_external_worker_receipts()?;
    let definition = contract
        .roles
        .iter()
        .find(|candidate| candidate.role == role)
        .cloned()
        .ok_or_else(|| CompiledAgentExternalWorkersError::InvalidContract {
            detail: format!("external worker role `{:?}` missing", role),
        })?;
    let receipt = receipts
        .receipts
        .iter()
        .find(|candidate| candidate.role == role)
        .cloned()
        .ok_or_else(|| CompiledAgentExternalWorkersError::InvalidReceipts {
            detail: format!("external worker receipt `{:?}` missing", role),
        })?;
    Ok((definition, receipt))
}

fn canonical_external_worker_source_artifacts(
) -> Result<Vec<CompiledAgentRoleArtifactRef>, CompiledAgentExternalWorkersError> {
    let internal_contract = canonical_compiled_agent_decentralized_roles_contract()?;
    let mut refs = internal_contract.source_artifacts.clone();
    refs.extend([
        load_artifact_ref(
            COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_FIXTURE_PATH,
            Some("contract_digest"),
            Some("contract_id"),
            "Governed internal decentralized-role contract that the external worker beta must match.",
        )?,
        load_artifact_ref(
            COMPILED_AGENT_DECENTRALIZED_ROLE_RECEIPTS_FIXTURE_PATH,
            Some("receipts_digest"),
            None,
            "Governed internal decentralized-role receipts that external worker outputs must stay compatible with.",
        )?,
        load_artifact_ref(
            COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_FIXTURE_PATH,
            Some("report_digest"),
            Some("dry_run_id"),
            "Retained internal dry run proving validator and rollback discipline before outside workers join.",
        )?,
        load_artifact_ref(
            COMPILED_AGENT_EXTERNAL_BENCHMARK_KIT_FIXTURE_PATH,
            Some("contract_digest"),
            Some("contract_id"),
            "Outside-compatible benchmark kit for the admitted compiled-agent family.",
        )?,
        load_artifact_ref(
            COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH,
            Some("run_digest"),
            Some("run_id"),
            "Retained external benchmark run on the admitted compiled-agent family.",
        )?,
        load_artifact_ref(
            COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH,
            Some("payload_digest"),
            Some("submission_id"),
            "Retained external runtime disagreement receipt submission in the governed intake shape.",
        )?,
        load_artifact_ref(
            COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH,
            Some("payload_digest"),
            Some("proposal_id"),
            "Retained external replay proposal emitted from quarantined external evidence.",
        )?,
        load_artifact_ref(
            COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH,
            Some("ledger_digest"),
            Some("ledger_id"),
            "Staging ledger that keeps accepted, rejected, and review-required external evidence separate from authority.",
        )?,
        load_artifact_ref(
            COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH,
            Some("report_digest"),
            Some("report_id"),
            "Quarantine report that keeps shadow assessments and replay-candidate linkage explicit for external evidence.",
        )?,
    ]);
    Ok(refs)
}

fn canonical_external_worker_roles(
    internal_contract: &crate::CompiledAgentDecentralizedRolesContract,
    source_artifacts: &[CompiledAgentRoleArtifactRef],
) -> Result<Vec<CompiledAgentExternalWorkerRoleDefinition>, CompiledAgentExternalWorkersError> {
    let benchmark_run = artifact_ref_from_set(
        source_artifacts,
        COMPILED_AGENT_EXTERNAL_BENCHMARK_RUN_FIXTURE_PATH,
    )?;
    let runtime_submission = artifact_ref_from_set(
        source_artifacts,
        COMPILED_AGENT_EXTERNAL_RUNTIME_RECEIPT_SUBMISSION_FIXTURE_PATH,
    )?;
    let replay_proposal = artifact_ref_from_set(
        source_artifacts,
        COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH,
    )?;
    let staging_ledger = artifact_ref_from_set(
        source_artifacts,
        COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH,
    )?;
    let quarantine_report = artifact_ref_from_set(
        source_artifacts,
        COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH,
    )?;

    Ok(vec![
        external_worker_role_definition(
            internal_role_definition(
                internal_contract,
                CompiledAgentDecentralizedRoleKind::ReplayGeneration,
            )?,
            "compiled_agent.external_worker.replay_generation.v1",
            "Allow an outside worker to normalize admitted external evidence into a replay proposal under the same bounded contract the internal loop already uses.",
            merge_manifest(
                "compiled_agent.external_worker.replay_generation.input.v1",
                &internal_role_definition(
                    internal_contract,
                    CompiledAgentDecentralizedRoleKind::ReplayGeneration,
                )?
                .input_manifest,
                vec![benchmark_run.clone(), staging_ledger.clone(), quarantine_report.clone()],
                vec![
                    String::from("submission_id"),
                    String::from("review_state"),
                    String::from("quarantine_status"),
                ],
                "External replay generation starts from the same learning-ledger logic plus admitted external staging and quarantine artifacts.",
            ),
            CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.external_worker.replay_generation.output.v1"),
                required_artifacts: vec![replay_proposal.clone()],
                expected_fields: vec![
                    String::from("proposal_id"),
                    String::from("source_receipt_id"),
                    String::from("proposed_samples"),
                    String::from("payload_digest"),
                ],
                detail: String::from(
                    "External replay generation emits a replay proposal that still requires governed review before it becomes replay authority.",
                ),
            },
            external_reference_path(
                "replay_generation",
                vec![String::from(COMPILED_AGENT_EXTERNAL_REPLAY_PROPOSAL_FIXTURE_PATH)],
                "Prints the outside-worker replay-generation job contract and retained submission receipt.",
            ),
            String::from(
                "Outside workers can generate replay proposals only. They do not bypass replay admission review or validator gates.",
            ),
        ),
        external_worker_role_definition(
            internal_role_definition(
                internal_contract,
                CompiledAgentDecentralizedRoleKind::RankingLabeling,
            )?,
            "compiled_agent.external_worker.ranking_labeling.v1",
            "Allow an outside worker to curate review-required external benchmark and runtime rows without bypassing the human-review boundary.",
            merge_manifest(
                "compiled_agent.external_worker.ranking_labeling.input.v1",
                &internal_role_definition(
                    internal_contract,
                    CompiledAgentDecentralizedRoleKind::RankingLabeling,
                )?
                .input_manifest,
                vec![benchmark_run.clone(), staging_ledger.clone(), quarantine_report.clone()],
                vec![
                    String::from("submission_id"),
                    String::from("review_required_receipt_ids"),
                    String::from("shadow_assessment_ids"),
                ],
                "External ranking-and-labeling uses the same narrow receipt and replay surfaces plus the retained intake artifacts.",
            ),
            CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.external_worker.ranking_labeling.output.v1"),
                required_artifacts: vec![staging_ledger.clone(), quarantine_report.clone()],
                expected_fields: vec![
                    String::from("review_state"),
                    String::from("curation_note"),
                    String::from("quarantine_status"),
                ],
                detail: String::from(
                    "External ranking-and-labeling emits governed review decisions back into the same staging and quarantine surfaces instead of inventing a new path.",
                ),
            },
            external_reference_path(
                "ranking_labeling",
                vec![
                    String::from(COMPILED_AGENT_EXTERNAL_SUBMISSION_STAGING_LEDGER_FIXTURE_PATH),
                    String::from(COMPILED_AGENT_EXTERNAL_QUARANTINE_REPORT_FIXTURE_PATH),
                ],
                "Prints the outside-worker ranking-and-labeling job contract and retained submission receipt.",
            ),
            String::from(
                "Outside workers can propose curation outcomes only. Human review still decides whether labels and corpus placement become authority.",
            ),
        ),
        external_worker_role_definition(
            internal_role_definition(
                internal_contract,
                CompiledAgentDecentralizedRoleKind::ValidatorScoring,
            )?,
            "compiled_agent.external_worker.validator_scoring.v1",
            "Allow an outside worker to score bounded candidates against the retained validator surfaces without granting promotion authority.",
            merge_manifest(
                "compiled_agent.external_worker.validator_scoring.input.v1",
                &internal_role_definition(
                    internal_contract,
                    CompiledAgentDecentralizedRoleKind::ValidatorScoring,
                )?
                .input_manifest,
                vec![runtime_submission.clone(), staging_ledger.clone(), quarantine_report.clone()],
                vec![
                    String::from("review_state"),
                    String::from("quarantine_status"),
                    String::from("source_submission_ids"),
                ],
                "External validator-scoring consumes the same bounded candidate and policy artifacts plus admitted external disagreement evidence.",
            ),
            internal_role_definition(
                internal_contract,
                CompiledAgentDecentralizedRoleKind::ValidatorScoring,
            )?
            .output_manifest
            .clone(),
            external_reference_path(
                "validator_scoring",
                vec![
                    String::from(COMPILED_AGENT_XTRAIN_CYCLE_RECEIPT_FIXTURE_PATH),
                    String::from(COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_FIXTURE_PATH),
                ],
                "Prints the outside-worker validator-scoring job contract and retained submission receipt.",
            ),
            String::from(
                "Outside workers can score bounded candidates only. They cannot directly move promotion state or runtime authority, even when their score packet is structurally valid.",
            ),
        ),
        external_worker_role_definition(
            internal_role_definition(
                internal_contract,
                CompiledAgentDecentralizedRoleKind::BoundedModuleTraining,
            )?,
            "compiled_agent.external_worker.bounded_module_training.v1",
            "Allow an outside worker to train bounded route and grounded candidate artifacts behind the same locked default row and replay contracts.",
            merge_manifest(
                "compiled_agent.external_worker.bounded_module_training.input.v1",
                &internal_role_definition(
                    internal_contract,
                    CompiledAgentDecentralizedRoleKind::BoundedModuleTraining,
                )?
                .input_manifest,
                vec![replay_proposal.clone(), quarantine_report.clone()],
                vec![
                    String::from("proposal_id"),
                    String::from("source_receipt_id"),
                    String::from("review_state"),
                ],
                "External bounded-module training stays on the same default row and replay bundle, but can also consume governed external replay proposals.",
            ),
            internal_role_definition(
                internal_contract,
                CompiledAgentDecentralizedRoleKind::BoundedModuleTraining,
            )?
            .output_manifest
            .clone(),
            external_reference_path(
                "bounded_module_training",
                vec![
                    String::from("fixtures/compiled_agent/compiled_agent_route_model_v1.json"),
                    String::from("fixtures/compiled_agent/compiled_agent_grounded_answer_model_v1.json"),
                    String::from("fixtures/compiled_agent/compiled_agent_route_tfidf_centroid_model_v1.json"),
                    String::from("fixtures/compiled_agent/compiled_agent_grounded_answer_tfidf_centroid_model_v1.json"),
                ],
                "Prints the outside-worker bounded-training job contract and retained submission receipt.",
            ),
            String::from(
                "Outside workers can train bounded artifacts only. Validator scoring still decides whether those artifacts can influence promotion.",
            ),
        ),
    ])
}

fn external_worker_role_definition(
    internal: CompiledAgentDecentralizedRoleDefinition,
    worker_role_id: &str,
    purpose: &str,
    input_manifest: CompiledAgentRoleManifest,
    output_manifest: CompiledAgentRoleManifest,
    local_reference_path: CompiledAgentRoleReferencePath,
    claim_boundary: String,
) -> CompiledAgentExternalWorkerRoleDefinition {
    CompiledAgentExternalWorkerRoleDefinition {
        role: internal.role,
        worker_role_id: String::from(worker_role_id),
        delegated_role_id: internal.role_id,
        purpose: String::from(purpose),
        input_manifest,
        output_manifest,
        local_reference_path,
        review_boundary: internal.review_boundary,
        validator_gate: internal.validator_gate,
        claim_boundary,
        detail: internal.detail,
    }
}

fn merge_manifest(
    manifest_id: &str,
    base: &CompiledAgentRoleManifest,
    extra_artifacts: Vec<CompiledAgentRoleArtifactRef>,
    extra_fields: Vec<String>,
    detail: &str,
) -> CompiledAgentRoleManifest {
    let mut required_artifacts = base.required_artifacts.clone();
    for artifact in extra_artifacts {
        if !required_artifacts
            .iter()
            .any(|candidate| candidate.artifact_ref == artifact.artifact_ref)
        {
            required_artifacts.push(artifact);
        }
    }
    let mut expected_fields = base.expected_fields.clone();
    for field in extra_fields {
        if !expected_fields.iter().any(|candidate| candidate == &field) {
            expected_fields.push(field);
        }
    }
    CompiledAgentRoleManifest {
        manifest_id: String::from(manifest_id),
        required_artifacts,
        expected_fields,
        detail: String::from(detail),
    }
}

fn internal_role_definition(
    contract: &crate::CompiledAgentDecentralizedRolesContract,
    role: CompiledAgentDecentralizedRoleKind,
) -> Result<CompiledAgentDecentralizedRoleDefinition, CompiledAgentExternalWorkersError> {
    contract
        .roles
        .iter()
        .find(|candidate| candidate.role == role)
        .cloned()
        .ok_or_else(|| CompiledAgentExternalWorkersError::InvalidContract {
            detail: format!("internal role `{:?}` missing", role),
        })
}

fn internal_role_receipt(
    receipts: &crate::CompiledAgentDecentralizedRoleReceipts,
    role: CompiledAgentDecentralizedRoleKind,
) -> crate::CompiledAgentDecentralizedRoleReceipt {
    receipts
        .receipts
        .iter()
        .find(|candidate| candidate.role == role)
        .cloned()
        .expect("internal decentralized role receipt must exist")
}

fn review_required_receipt_ids(
    staging_ledger: &crate::CompiledAgentExternalSubmissionStagingLedger,
) -> Vec<String> {
    let mut ids = staging_ledger
        .submissions
        .iter()
        .flat_map(|record| record.review_required_receipt_ids.clone())
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn artifact_ref_from_set(
    artifact_set: &[CompiledAgentRoleArtifactRef],
    artifact_ref: &str,
) -> Result<CompiledAgentRoleArtifactRef, CompiledAgentExternalWorkersError> {
    artifact_set
        .iter()
        .find(|artifact| artifact.artifact_ref == artifact_ref)
        .cloned()
        .ok_or_else(|| CompiledAgentExternalWorkersError::InvalidContract {
            detail: format!("source artifact `{artifact_ref}` missing"),
        })
}

fn load_artifact_ref(
    artifact_ref: &str,
    digest_field: Option<&str>,
    identity_field: Option<&str>,
    detail: &str,
) -> Result<CompiledAgentRoleArtifactRef, CompiledAgentExternalWorkersError> {
    let path = repo_relative_path(artifact_ref);
    let value: Value = serde_json::from_slice(&fs::read(&path).map_err(|error| {
        CompiledAgentExternalWorkersError::Read {
            path: path.display().to_string(),
            error,
        }
    })?)?;
    let schema_version = value
        .get("schema_version")
        .map(value_to_string)
        .unwrap_or_else(|| String::from("unknown"));
    let evidence_class = value
        .get("evidence_class")
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map_err(CompiledAgentExternalWorkersError::Json)?;
    let artifact_digest = digest_field
        .and_then(|field| value.get(field))
        .map(value_to_string);
    if digest_field.is_some() && artifact_digest.as_deref().unwrap_or_default().is_empty() {
        return Err(CompiledAgentExternalWorkersError::InvalidContract {
            detail: format!("artifact `{artifact_ref}` lost required digest field"),
        });
    }
    let identity_value = identity_field
        .and_then(|field| value.get(field))
        .map(value_to_string);
    if identity_field.is_some() && identity_value.as_deref().unwrap_or_default().is_empty() {
        return Err(CompiledAgentExternalWorkersError::InvalidContract {
            detail: format!("artifact `{artifact_ref}` lost required identity field"),
        });
    }
    Ok(CompiledAgentRoleArtifactRef {
        artifact_ref: String::from(artifact_ref),
        schema_version,
        evidence_class,
        digest_field: digest_field.map(String::from),
        artifact_digest,
        identity_field: identity_field.map(String::from),
        identity_value,
        detail: String::from(detail),
    })
}

fn external_reference_path(
    role_selector: &str,
    retained_output_refs: Vec<String>,
    detail: &str,
) -> CompiledAgentRoleReferencePath {
    CompiledAgentRoleReferencePath {
        command: format!(
            "cargo run -q -p psionic-train --bin compiled_agent_external_workers -- --role {role_selector}"
        ),
        bin_path: String::from(COMPILED_AGENT_EXTERNAL_WORKERS_BIN_PATH),
        role_selector: String::from(role_selector),
        retained_output_refs,
        detail: String::from(detail),
    }
}

fn write_json<T: Serialize + Clone>(
    output_path: impl AsRef<Path>,
    value: &T,
) -> Result<T, CompiledAgentExternalWorkersError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CompiledAgentExternalWorkersError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentExternalWorkersError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(value.clone())
}

fn verify_fixture<T: for<'de> Deserialize<'de> + PartialEq>(
    path: PathBuf,
    expected: T,
) -> Result<(), CompiledAgentExternalWorkersError> {
    let bytes = fs::read(&path).map_err(|error| CompiledAgentExternalWorkersError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let committed: T = serde_json::from_slice(&bytes)?;
    if committed != expected {
        return Err(CompiledAgentExternalWorkersError::FixtureDrift {
            path: path.display().to_string(),
        });
    }
    Ok(())
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(string) => string.clone(),
        _ => value.to_string(),
    }
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let canonical =
        serde_json::to_vec(value).expect("stable digest serialization must succeed");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&canonical);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_compiled_agent_external_worker_beta_contract,
        canonical_compiled_agent_external_worker_dry_run_report,
        canonical_compiled_agent_external_worker_receipts,
        verify_compiled_agent_external_worker_fixtures,
    };

    #[test]
    fn compiled_agent_external_worker_contract_is_valid(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_external_worker_beta_contract()?;
        contract.validate()?;
        assert_eq!(contract.roles.len(), 4);
        Ok(())
    }

    #[test]
    fn compiled_agent_external_worker_receipts_are_valid(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_external_worker_beta_contract()?;
        let receipts = canonical_compiled_agent_external_worker_receipts()?;
        receipts.validate(&contract)?;
        assert!(receipts
            .receipts
            .iter()
            .any(|receipt| receipt.review_state == crate::CompiledAgentExternalReviewState::Rejected));
        Ok(())
    }

    #[test]
    fn compiled_agent_external_worker_dry_run_is_valid(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = canonical_compiled_agent_external_worker_dry_run_report()?;
        assert!(report.validator_discipline_unchanged);
        assert!(report.rollback_discipline_unchanged);
        Ok(())
    }

    #[test]
    fn compiled_agent_external_worker_fixtures_match_canonical_output(
    ) -> Result<(), Box<dyn std::error::Error>> {
        verify_compiled_agent_external_worker_fixtures()?;
        Ok(())
    }
}
