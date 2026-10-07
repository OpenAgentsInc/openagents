use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{CompiledAgentEvidenceClass, CompiledAgentModuleKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    COMPILED_AGENT_RUNTIME_FIXTURE_DIR, CompiledAgentArtifactContractError,
    CompiledAgentCandidateFamilyDecision, CompiledAgentPromotionDecision,
    CompiledAgentReceiptError, CompiledAgentShadowGovernanceError, CompiledAgentXtrainError,
    canonical_compiled_agent_confidence_policy, canonical_compiled_agent_learning_receipt_ledger,
    canonical_compiled_agent_promoted_artifact_contract, canonical_compiled_agent_replay_bundle,
    canonical_compiled_agent_runtime_source_receipts,
    canonical_compiled_agent_shadow_disagreement_receipts,
    canonical_compiled_agent_stronger_candidate_family_report,
    canonical_compiled_agent_xtrain_cycle_receipt, repo_relative_path,
};

pub const COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.decentralized_roles_contract.v1";
pub const COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.decentralized_role_receipts.v1";
pub const COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.decentralized_role_dry_run.v1";
pub const COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_ID: &str =
    "compiled_agent.decentralized_role_dry_run.v1";
pub const COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_ID: &str =
    "compiled_agent.decentralized_roles.contract.v1";
pub const COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_decentralized_roles_contract_v1.json";
pub const COMPILED_AGENT_DECENTRALIZED_ROLE_RECEIPTS_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_decentralized_role_receipts_v1.json";
pub const COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_decentralized_role_dry_run_v1.json";
pub const COMPILED_AGENT_DECENTRALIZED_ROLES_BIN_PATH: &str =
    "crates/psionic-train/src/bin/compiled_agent_decentralized_roles.rs";
pub const COMPILED_AGENT_DECENTRALIZED_ROLES_DOC_PATH: &str =
    "docs/COMPILED_AGENT_DECENTRALIZED_ROLES.md";

const LEARNING_RECEIPTS_REF: &str =
    "fixtures/compiled_agent/compiled_agent_learning_receipts_v1.json";
const REPLAY_BUNDLE_REF: &str = "fixtures/compiled_agent/compiled_agent_replay_bundle_v1.json";
const DEFAULT_ROW_REF: &str = "fixtures/compiled_agent/compiled_agent_default_row_v1.json";
const ROUTE_MODEL_REF: &str = "fixtures/compiled_agent/compiled_agent_route_model_v1.json";
const GROUNDED_MODEL_REF: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_answer_model_v1.json";
const ROUTE_REPORT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_route_candidate_module_eval_report_v1.json";
const GROUNDED_REPORT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_candidate_module_eval_report_v1.json";
const ROUTE_STRONGER_MODEL_REF: &str =
    "fixtures/compiled_agent/compiled_agent_route_tfidf_centroid_model_v1.json";
const GROUNDED_STRONGER_MODEL_REF: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_answer_tfidf_centroid_model_v1.json";
const ROUTE_STRONGER_REPORT_REF: &str = "fixtures/compiled_agent/compiled_agent_route_tfidf_centroid_candidate_module_eval_report_v1.json";
const GROUNDED_STRONGER_REPORT_REF: &str = "fixtures/compiled_agent/compiled_agent_grounded_tfidf_centroid_candidate_module_eval_report_v1.json";
const STRONGER_CANDIDATE_FAMILY_REPORT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_stronger_candidate_family_report_v1.json";
const XTRAIN_RECEIPT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_xtrain_cycle_receipt_v1.json";
const PROMOTED_CONTRACT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_promoted_artifact_contract_v1.json";
const CONFIDENCE_POLICY_REF: &str =
    "fixtures/compiled_agent/compiled_agent_confidence_policy_v1.json";
const SHADOW_DISAGREEMENTS_REF: &str =
    "fixtures/compiled_agent/compiled_agent_shadow_disagreement_receipts_v1.json";

#[derive(Debug, Error)]
pub enum CompiledAgentDecentralizedRolesError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("invalid decentralized roles contract: {detail}")]
    InvalidContract { detail: String },
    #[error("invalid decentralized role receipts: {detail}")]
    InvalidReceipts { detail: String },
    #[error("fixture `{path}` drifted from the canonical generator output")]
    FixtureDrift { path: String },
    #[error(
        "compiled-agent evidence class drifted in `{context}`: expected `{expected:?}` but found `{actual:?}`"
    )]
    MixedEvidenceClass {
        context: String,
        expected: CompiledAgentEvidenceClass,
        actual: CompiledAgentEvidenceClass,
    },
    #[error(transparent)]
    ArtifactContract(#[from] CompiledAgentArtifactContractError),
    #[error(transparent)]
    Receipts(#[from] CompiledAgentReceiptError),
    #[error(transparent)]
    ShadowGovernance(#[from] CompiledAgentShadowGovernanceError),
    #[error(transparent)]
    Xtrain(#[from] CompiledAgentXtrainError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentDecentralizedRoleKind {
    ReplayGeneration,
    RankingLabeling,
    ValidatorScoring,
    BoundedModuleTraining,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentRoleReviewBoundary {
    HumanReviewRequired,
    ValidatorGateRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentRoleReceiptStatus {
    AcceptedAfterHumanReview,
    AcceptedAfterValidatorGate,
    QueuedForValidatorScoring,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRoleArtifactRef {
    pub artifact_ref: String,
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_class: Option<CompiledAgentEvidenceClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_value: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRoleManifest {
    pub manifest_id: String,
    pub required_artifacts: Vec<CompiledAgentRoleArtifactRef>,
    pub expected_fields: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentRoleReferencePath {
    pub command: String,
    pub bin_path: String,
    pub role_selector: String,
    pub retained_output_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDecentralizedRoleDefinition {
    pub role: CompiledAgentDecentralizedRoleKind,
    pub role_id: String,
    pub purpose: String,
    pub input_manifest: CompiledAgentRoleManifest,
    pub output_manifest: CompiledAgentRoleManifest,
    pub receipt_schema_version: String,
    pub local_reference_path: CompiledAgentRoleReferencePath,
    pub review_boundary: CompiledAgentRoleReviewBoundary,
    pub validator_gate: String,
    pub claim_boundary: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDecentralizedRolesAuthorityPaths {
    pub contract_fixture_path: String,
    pub receipts_fixture_path: String,
    pub bin_path: String,
    pub doc_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDecentralizedRolesContract {
    pub schema_version: String,
    pub contract_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub source_artifacts: Vec<CompiledAgentRoleArtifactRef>,
    pub roles: Vec<CompiledAgentDecentralizedRoleDefinition>,
    pub authority_paths: CompiledAgentDecentralizedRolesAuthorityPaths,
    pub claim_boundary: String,
    pub contract_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDecentralizedRoleReceipt {
    pub role: CompiledAgentDecentralizedRoleKind,
    pub receipt_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub status: CompiledAgentRoleReceiptStatus,
    pub input_refs: Vec<String>,
    pub output_refs: Vec<String>,
    pub emitted_ids: Vec<String>,
    pub next_consumer: String,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDecentralizedRoleReceipts {
    pub schema_version: String,
    pub contract_digest: String,
    pub receipts: Vec<CompiledAgentDecentralizedRoleReceipt>,
    pub summary: String,
    pub receipts_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDecentralizedDryRunStep {
    pub order: u32,
    pub role: CompiledAgentDecentralizedRoleKind,
    pub role_id: String,
    pub receipt_id: String,
    pub status: CompiledAgentRoleReceiptStatus,
    pub review_boundary: CompiledAgentRoleReviewBoundary,
    pub validator_gate: String,
    pub consumed_artifacts: Vec<CompiledAgentRoleArtifactRef>,
    pub produced_artifacts: Vec<CompiledAgentRoleArtifactRef>,
    pub runtime_fixture_refs: Vec<String>,
    pub runtime_learning_receipt_ids: Vec<String>,
    pub emitted_ids: Vec<String>,
    pub next_consumer: String,
    pub lineage_intact: bool,
    pub validator_discipline_preserved: bool,
    pub rollback_discipline_preserved: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentDecentralizedRoleDryRunReport {
    pub schema_version: String,
    pub dry_run_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub contract_digest: String,
    pub receipts_digest: String,
    pub learning_ledger_digest: String,
    pub replay_bundle_digest: String,
    pub xtrain_receipt_digest: String,
    pub promoted_contract_digest: String,
    pub confidence_policy_digest: String,
    pub shadow_disagreement_receipts_digest: String,
    pub stronger_candidate_family_report_digest: String,
    pub runtime_fixture_refs: Vec<String>,
    pub runtime_learning_receipt_ids: Vec<String>,
    pub role_runs: Vec<CompiledAgentDecentralizedDryRunStep>,
    pub validator_discipline_unchanged: bool,
    pub rollback_discipline_unchanged: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

impl CompiledAgentDecentralizedRolesContract {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.contract_digest.clear();
        stable_digest(b"compiled_agent_decentralized_roles_contract|", &clone)
    }

    pub fn validate(&self) -> Result<(), CompiledAgentDecentralizedRolesError> {
        if self.schema_version != COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_SCHEMA_VERSION {
            return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_id != COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_ID {
            return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
                detail: String::from("contract_id drifted"),
            });
        }
        if self.authority_paths.contract_fixture_path
            != COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_FIXTURE_PATH
            || self.authority_paths.receipts_fixture_path
                != COMPILED_AGENT_DECENTRALIZED_ROLE_RECEIPTS_FIXTURE_PATH
            || self.authority_paths.bin_path != COMPILED_AGENT_DECENTRALIZED_ROLES_BIN_PATH
            || self.authority_paths.doc_path != COMPILED_AGENT_DECENTRALIZED_ROLES_DOC_PATH
        {
            return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
                detail: String::from("authority paths drifted"),
            });
        }
        if self.roles.len() != 4 {
            return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
                detail: String::from("expected exactly four decentralized roles"),
            });
        }

        let role_ids = self
            .roles
            .iter()
            .map(|role| role.role_id.as_str())
            .collect::<BTreeSet<_>>();
        if role_ids.len() != self.roles.len() {
            return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
                detail: String::from("role ids must stay unique"),
            });
        }

        let expected_refs = canonical_source_artifacts()?;
        if self.source_artifacts != expected_refs {
            return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
                detail: String::from("source artifact refs drifted"),
            });
        }
        for artifact in &self.source_artifacts {
            if let Some(actual) = artifact.evidence_class {
                ensure_matching_evidence_class(
                    self.evidence_class,
                    actual,
                    format!(
                        "compiled_agent_decentralized_roles_contract source {}",
                        artifact.artifact_ref
                    ),
                )?;
            }
        }

        for role in &self.roles {
            if role.receipt_schema_version
                != COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION
                || role.local_reference_path.bin_path != COMPILED_AGENT_DECENTRALIZED_ROLES_BIN_PATH
                || !role
                    .local_reference_path
                    .command
                    .contains("compiled_agent_decentralized_roles")
                || role.input_manifest.required_artifacts.is_empty()
                || role.output_manifest.required_artifacts.is_empty()
            {
                return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
                    detail: format!("role `{}` lost its governed reference path", role.role_id),
                });
            }
        }

        if self.contract_digest != self.stable_digest() {
            return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
                detail: String::from("contract digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentDecentralizedRoleReceipts {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.receipts_digest.clear();
        stable_digest(b"compiled_agent_decentralized_role_receipts|", &clone)
    }

    pub fn validate(
        &self,
        contract: &CompiledAgentDecentralizedRolesContract,
    ) -> Result<(), CompiledAgentDecentralizedRolesError> {
        if self.schema_version != COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: format!(
                    "schema_version must stay `{}` but was `{}`",
                    COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.contract_digest != contract.contract_digest {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("role receipts lost contract linkage"),
            });
        }
        if self.receipts.len() != contract.roles.len() {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("receipt count no longer matches role count"),
            });
        }
        let role_set = self
            .receipts
            .iter()
            .map(|receipt| receipt.role)
            .collect::<BTreeSet<_>>();
        if role_set.len() != self.receipts.len() {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("each role must keep exactly one retained local receipt"),
            });
        }

        for receipt in &self.receipts {
            ensure_matching_evidence_class(
                contract.evidence_class,
                receipt.evidence_class,
                format!(
                    "compiled_agent_decentralized_role_receipt {}",
                    receipt.receipt_id
                ),
            )?;
            let role = contract
                .roles
                .iter()
                .find(|role| role.role == receipt.role)
                .ok_or_else(|| CompiledAgentDecentralizedRolesError::InvalidReceipts {
                    detail: format!("receipt `{}` lost role linkage", receipt.receipt_id),
                })?;

            let expected_inputs = role
                .input_manifest
                .required_artifacts
                .iter()
                .map(|artifact| artifact.artifact_ref.as_str())
                .collect::<BTreeSet<_>>();
            let expected_outputs = role
                .output_manifest
                .required_artifacts
                .iter()
                .map(|artifact| artifact.artifact_ref.as_str())
                .collect::<BTreeSet<_>>();

            if receipt
                .input_refs
                .iter()
                .any(|artifact_ref| !expected_inputs.contains(artifact_ref.as_str()))
                || receipt
                    .output_refs
                    .iter()
                    .any(|artifact_ref| !expected_outputs.contains(artifact_ref.as_str()))
            {
                return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                    detail: format!(
                        "receipt `{}` drifted from role `{}` artifact lineage",
                        receipt.receipt_id, role.role_id
                    ),
                });
            }
        }

        if self.receipts_digest != self.stable_digest() {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("receipts digest drifted"),
            });
        }
        Ok(())
    }
}

impl CompiledAgentDecentralizedRoleDryRunReport {
    #[must_use]
    pub fn stable_digest(&self) -> String {
        let mut clone = self.clone();
        clone.report_digest.clear();
        stable_digest(b"compiled_agent_decentralized_role_dry_run|", &clone)
    }

    pub fn validate(
        &self,
        contract: &CompiledAgentDecentralizedRolesContract,
        receipts: &CompiledAgentDecentralizedRoleReceipts,
    ) -> Result<(), CompiledAgentDecentralizedRolesError> {
        if self.schema_version != COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_SCHEMA_VERSION {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: format!(
                    "dry-run schema_version must stay `{}` but was `{}`",
                    COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_SCHEMA_VERSION, self.schema_version
                ),
            });
        }
        if self.dry_run_id != COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_ID {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("dry-run id drifted"),
            });
        }
        if self.evidence_class != contract.evidence_class {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("dry-run report drifted evidence class"),
            });
        }
        if self.contract_digest != contract.contract_digest
            || self.receipts_digest != receipts.receipts_digest
        {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("dry-run report lost contract or receipt linkage"),
            });
        }
        if self.role_runs.len() != contract.roles.len() {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("dry-run report lost one or more governed roles"),
            });
        }
        if !self.validator_discipline_unchanged || !self.rollback_discipline_unchanged {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("dry-run report weakened validator or rollback discipline"),
            });
        }

        let runtime_fixture_set = self
            .runtime_fixture_refs
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let runtime_receipt_set = self
            .runtime_learning_receipt_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();

        for step in &self.role_runs {
            let role = contract
                .roles
                .iter()
                .find(|candidate| candidate.role == step.role)
                .ok_or_else(|| CompiledAgentDecentralizedRolesError::InvalidReceipts {
                    detail: format!("dry-run step `{}` lost role linkage", step.receipt_id),
                })?;
            let receipt = receipts
                .receipts
                .iter()
                .find(|candidate| candidate.role == step.role)
                .ok_or_else(|| CompiledAgentDecentralizedRolesError::InvalidReceipts {
                    detail: format!("dry-run step `{}` lost receipt linkage", step.receipt_id),
                })?;

            if step.role_id != role.role_id
                || step.receipt_id != receipt.receipt_id
                || step.status != receipt.status
                || step.review_boundary != role.review_boundary
                || step.validator_gate != role.validator_gate
                || step.consumed_artifacts != role.input_manifest.required_artifacts
                || step.produced_artifacts != role.output_manifest.required_artifacts
                || step.emitted_ids != receipt.emitted_ids
                || step.next_consumer != receipt.next_consumer
                || !step.lineage_intact
                || !step.validator_discipline_preserved
                || !step.rollback_discipline_preserved
                || step
                    .runtime_fixture_refs
                    .iter()
                    .any(|reference| !runtime_fixture_set.contains(reference.as_str()))
                || step
                    .runtime_learning_receipt_ids
                    .iter()
                    .any(|receipt_id| !runtime_receipt_set.contains(receipt_id.as_str()))
            {
                return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                    detail: format!(
                        "dry-run step `{}` drifted from the governed contract",
                        step.receipt_id
                    ),
                });
            }
        }

        if self.report_digest != self.stable_digest() {
            return Err(CompiledAgentDecentralizedRolesError::InvalidReceipts {
                detail: String::from("dry-run digest drifted"),
            });
        }
        Ok(())
    }
}

#[must_use]
pub fn compiled_agent_decentralized_roles_contract_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_decentralized_role_receipts_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_DECENTRALIZED_ROLE_RECEIPTS_FIXTURE_PATH)
}

#[must_use]
pub fn compiled_agent_decentralized_role_dry_run_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_FIXTURE_PATH)
}

pub fn canonical_compiled_agent_decentralized_roles_contract()
-> Result<CompiledAgentDecentralizedRolesContract, CompiledAgentDecentralizedRolesError> {
    let source_artifacts = canonical_source_artifacts()?;
    let learning_receipts = artifact_ref_from_set(&source_artifacts, LEARNING_RECEIPTS_REF)?;
    let replay_bundle = artifact_ref_from_set(&source_artifacts, REPLAY_BUNDLE_REF)?;
    let default_row = artifact_ref_from_set(&source_artifacts, DEFAULT_ROW_REF)?;
    let route_model = artifact_ref_from_set(&source_artifacts, ROUTE_MODEL_REF)?;
    let grounded_model = artifact_ref_from_set(&source_artifacts, GROUNDED_MODEL_REF)?;
    let route_report = artifact_ref_from_set(&source_artifacts, ROUTE_REPORT_REF)?;
    let grounded_report = artifact_ref_from_set(&source_artifacts, GROUNDED_REPORT_REF)?;
    let route_stronger_model = artifact_ref_from_set(&source_artifacts, ROUTE_STRONGER_MODEL_REF)?;
    let grounded_stronger_model =
        artifact_ref_from_set(&source_artifacts, GROUNDED_STRONGER_MODEL_REF)?;
    let route_stronger_report =
        artifact_ref_from_set(&source_artifacts, ROUTE_STRONGER_REPORT_REF)?;
    let grounded_stronger_report =
        artifact_ref_from_set(&source_artifacts, GROUNDED_STRONGER_REPORT_REF)?;
    let stronger_family_report =
        artifact_ref_from_set(&source_artifacts, STRONGER_CANDIDATE_FAMILY_REPORT_REF)?;
    let xtrain_receipt = artifact_ref_from_set(&source_artifacts, XTRAIN_RECEIPT_REF)?;
    let promoted_contract = artifact_ref_from_set(&source_artifacts, PROMOTED_CONTRACT_REF)?;
    let confidence_policy = artifact_ref_from_set(&source_artifacts, CONFIDENCE_POLICY_REF)?;
    let shadow_disagreements = artifact_ref_from_set(&source_artifacts, SHADOW_DISAGREEMENTS_REF)?;

    let roles = vec![
        CompiledAgentDecentralizedRoleDefinition {
            role: CompiledAgentDecentralizedRoleKind::ReplayGeneration,
            role_id: String::from("compiled_agent.role.replay_generation.v1"),
            purpose: String::from(
                "Normalize accepted compiled-agent learning receipts into bounded replay samples for route and grounded-answer training.",
            ),
            input_manifest: CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.replay_generation.input.v1"),
                required_artifacts: vec![learning_receipts.clone()],
                expected_fields: vec![
                    String::from("receipt_id"),
                    String::from("expected_route"),
                    String::from("expected_public_response"),
                    String::from("failure_classes"),
                    String::from("corpus_split"),
                ],
                detail: String::from(
                    "Replay generation starts from governed learning receipts only. It does not scrape arbitrary logs or create unlabeled rows outside the compiled-agent receipt schema.",
                ),
            },
            output_manifest: CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.replay_generation.output.v1"),
                required_artifacts: vec![replay_bundle.clone()],
                expected_fields: vec![
                    String::from("sample_id"),
                    String::from("module"),
                    String::from("input"),
                    String::from("expected_output"),
                    String::from("tags"),
                    String::from("bundle_digest"),
                ],
                detail: String::from(
                    "Replay generation emits replay rows that can feed bounded route and grounded-answer training directly.",
                ),
            },
            receipt_schema_version: String::from(
                COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION,
            ),
            local_reference_path: local_reference_path(
                "replay_generation",
                vec![String::from(REPLAY_BUNDLE_REF)],
                "Writes the canonical compiled-agent replay bundle from the retained learning ledger.",
            ),
            review_boundary: CompiledAgentRoleReviewBoundary::HumanReviewRequired,
            validator_gate: String::from(
                "Replay bundle digest and sample lineage must stay aligned with the retained learning ledger before rows can enter training.",
            ),
            claim_boundary: String::from(
                "This role proposes replay rows only. It does not promote candidates or bypass the receipt review path.",
            ),
            detail: String::from(
                "This is the first decentralized role because replay generation is the narrowest way to turn usage into governed training material.",
            ),
        },
        CompiledAgentDecentralizedRoleDefinition {
            role: CompiledAgentDecentralizedRoleKind::RankingLabeling,
            role_id: String::from("compiled_agent.role.ranking_labeling.v1"),
            purpose: String::from(
                "Curate receipt-backed route and grounded-answer labels before they are admitted into replay or held-out validator sets.",
            ),
            input_manifest: CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.ranking_labeling.input.v1"),
                required_artifacts: vec![learning_receipts.clone(), replay_bundle.clone()],
                expected_fields: vec![
                    String::from("receipt_id"),
                    String::from("operator_note"),
                    String::from("failure_classes"),
                    String::from("sample_id"),
                    String::from("tags"),
                ],
                detail: String::from(
                    "Ranking and labeling only touches retained receipts and replay rows that already exist inside the narrow compiled-agent scope.",
                ),
            },
            output_manifest: CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.ranking_labeling.output.v1"),
                required_artifacts: vec![learning_receipts.clone(), replay_bundle.clone()],
                expected_fields: vec![
                    String::from("accepted_label"),
                    String::from("corpus_split"),
                    String::from("review_decision"),
                    String::from("curation_note"),
                ],
                detail: String::from(
                    "Ranking and labeling emits curated receipt decisions that feed later replay generation and held-out scoring without claiming autonomous labeling authority.",
                ),
            },
            receipt_schema_version: String::from(
                COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION,
            ),
            local_reference_path: local_reference_path(
                "ranking_labeling",
                vec![
                    String::from(LEARNING_RECEIPTS_REF),
                    String::from(REPLAY_BUNDLE_REF),
                ],
                "Prints the retained ranking-and-labeling role definition and receipt against the current learning ledger and replay bundle.",
            ),
            review_boundary: CompiledAgentRoleReviewBoundary::HumanReviewRequired,
            validator_gate: String::from(
                "Human acceptance still decides whether a label becomes training data, held-out truth, or a rejected proposal.",
            ),
            claim_boundary: String::from(
                "This role curates narrow labels and corpus placement only. It does not autonomously widen tasks or invent new reward surfaces.",
            ),
            detail: String::from(
                "This role keeps the first decentralized human-in-the-loop curation surface explicit instead of hiding it in ad hoc notebook work.",
            ),
        },
        CompiledAgentDecentralizedRoleDefinition {
            role: CompiledAgentDecentralizedRoleKind::ValidatorScoring,
            role_id: String::from("compiled_agent.role.validator_scoring.v1"),
            purpose: String::from(
                "Score bounded candidate modules against replay, held-out rows, and independent module eval surfaces before promotion.",
            ),
            input_manifest: CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.validator_scoring.input.v1"),
                required_artifacts: vec![
                    route_model.clone(),
                    grounded_model.clone(),
                    route_report.clone(),
                    grounded_report.clone(),
                    route_stronger_model.clone(),
                    grounded_stronger_model.clone(),
                    route_stronger_report.clone(),
                    grounded_stronger_report.clone(),
                    stronger_family_report.clone(),
                    xtrain_receipt.clone(),
                    confidence_policy.clone(),
                    shadow_disagreements.clone(),
                ],
                expected_fields: vec![
                    String::from("artifact_digest"),
                    String::from("candidate_revision_id"),
                    String::from("report_digest"),
                    String::from("baseline_passed_cases"),
                    String::from("candidate_passed_cases"),
                    String::from("decision"),
                    String::from("receipts_digest"),
                ],
                detail: String::from(
                    "Validator scoring stays bounded to explicit incumbent-versus-stronger candidate artifacts, independent validator surfaces, the current confidence policy, and retained shadow-disagreement receipts. It does not freehand product judgments.",
                ),
            },
            output_manifest: CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.validator_scoring.output.v1"),
                required_artifacts: vec![xtrain_receipt.clone(), promoted_contract.clone()],
                expected_fields: vec![
                    String::from("decision"),
                    String::from("improvement_case_ids"),
                    String::from("heldout_match_count"),
                    String::from("contract_digest"),
                ],
                detail: String::from(
                    "Validator scoring emits bounded promote-or-hold outcomes and the runtime-consumable promoted-artifact contract.",
                ),
            },
            receipt_schema_version: String::from(
                COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION,
            ),
            local_reference_path: local_reference_path(
                "validator_scoring",
                vec![
                    String::from(ROUTE_REPORT_REF),
                    String::from(GROUNDED_REPORT_REF),
                    String::from(ROUTE_STRONGER_REPORT_REF),
                    String::from(GROUNDED_STRONGER_REPORT_REF),
                    String::from(STRONGER_CANDIDATE_FAMILY_REPORT_REF),
                    String::from(XTRAIN_RECEIPT_REF),
                    String::from(PROMOTED_CONTRACT_REF),
                    String::from(CONFIDENCE_POLICY_REF),
                    String::from(SHADOW_DISAGREEMENTS_REF),
                ],
                "Prints the retained validator-scoring role definition and receipt against the current candidate reports, stronger-family report, confidence policy, shadow disagreements, XTRAIN receipt, and promoted contract.",
            ),
            review_boundary: CompiledAgentRoleReviewBoundary::ValidatorGateRequired,
            validator_gate: String::from(
                "Independent module eval plus held-out replay scoring must stay non-regressing before promotion can move runtime authority.",
            ),
            claim_boundary: String::from(
                "This role scores bounded candidates only. It does not skip rollback authority or widen into open-ended swarm promotion.",
            ),
            detail: String::from(
                "This role is the first place decentralized work can influence promotion, but only through explicit validator-scored receipts.",
            ),
        },
        CompiledAgentDecentralizedRoleDefinition {
            role: CompiledAgentDecentralizedRoleKind::BoundedModuleTraining,
            role_id: String::from("compiled_agent.role.bounded_module_training.v1"),
            purpose: String::from(
                "Train narrow compiled-agent candidate modules from curated replay on the locked default learned row.",
            ),
            input_manifest: CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.bounded_module_training.input.v1"),
                required_artifacts: vec![
                    default_row.clone(),
                    learning_receipts.clone(),
                    replay_bundle.clone(),
                ],
                expected_fields: vec![
                    String::from("row_id"),
                    String::from("replay_bundle_digest"),
                    String::from("sample_id"),
                    String::from("module"),
                    String::from("expected_output"),
                ],
                detail: String::from(
                    "Bounded module training is locked to the current default learned row and the retained replay bundle. It does not claim arbitrary base-model mutation.",
                ),
            },
            output_manifest: CompiledAgentRoleManifest {
                manifest_id: String::from("compiled_agent.bounded_module_training.output.v1"),
                required_artifacts: vec![
                    route_model.clone(),
                    grounded_model.clone(),
                    route_stronger_model.clone(),
                    grounded_stronger_model.clone(),
                ],
                expected_fields: vec![
                    String::from("artifact_id"),
                    String::from("artifact_digest"),
                    String::from("training_accuracy"),
                    String::from("heldout_accuracy"),
                ],
                detail: String::from(
                    "Bounded module training emits replay-trained incumbent and stronger candidate artifacts that feed validator scoring and later promotion gates.",
                ),
            },
            receipt_schema_version: String::from(
                COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION,
            ),
            local_reference_path: local_reference_path(
                "bounded_module_training",
                vec![
                    String::from(ROUTE_MODEL_REF),
                    String::from(GROUNDED_MODEL_REF),
                    String::from(ROUTE_STRONGER_MODEL_REF),
                    String::from(GROUNDED_STRONGER_MODEL_REF),
                ],
                "Prints the retained bounded-module-training role definition and receipt against the current incumbent and stronger route and grounded model artifacts.",
            ),
            review_boundary: CompiledAgentRoleReviewBoundary::ValidatorGateRequired,
            validator_gate: String::from(
                "Candidate artifacts remain queued for validator scoring until replay, held-out, and independent-module gates are rechecked.",
            ),
            claim_boundary: String::from(
                "This role trains narrow module artifacts only. It does not rewrite the full runtime or bypass promotion review.",
            ),
            detail: String::from(
                "This role is the first bounded decentralized training surface because it maps cleanly onto the current route and grounded modules.",
            ),
        },
    ];

    let mut contract = CompiledAgentDecentralizedRolesContract {
        schema_version: String::from(COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_SCHEMA_VERSION),
        contract_id: String::from(COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_ID),
        evidence_class: CompiledAgentEvidenceClass::LearnedLane,
        source_artifacts,
        roles,
        authority_paths: CompiledAgentDecentralizedRolesAuthorityPaths {
            contract_fixture_path: String::from(
                COMPILED_AGENT_DECENTRALIZED_ROLES_CONTRACT_FIXTURE_PATH,
            ),
            receipts_fixture_path: String::from(
                COMPILED_AGENT_DECENTRALIZED_ROLE_RECEIPTS_FIXTURE_PATH,
            ),
            bin_path: String::from(COMPILED_AGENT_DECENTRALIZED_ROLES_BIN_PATH),
            doc_path: String::from(COMPILED_AGENT_DECENTRALIZED_ROLES_DOC_PATH),
        },
        claim_boundary: String::from(
            "These are pre-network governed compiled-agent improvement roles only. They operate on retained receipts, replay bundles, validator reports, and bounded candidate artifacts. They do not claim broad autonomous worker behavior, public incentives, or validator-free promotion.",
        ),
        contract_digest: String::new(),
    };
    contract.contract_digest = contract.stable_digest();
    contract.validate()?;
    Ok(contract)
}

pub fn canonical_compiled_agent_decentralized_role_receipts()
-> Result<CompiledAgentDecentralizedRoleReceipts, CompiledAgentDecentralizedRolesError> {
    let contract = canonical_compiled_agent_decentralized_roles_contract()?;
    let receipts = vec![
        CompiledAgentDecentralizedRoleReceipt {
            role: CompiledAgentDecentralizedRoleKind::ReplayGeneration,
            receipt_id: String::from("compiled_agent.role_receipt.replay_generation.v1"),
            evidence_class: contract.evidence_class,
            status: CompiledAgentRoleReceiptStatus::AcceptedAfterHumanReview,
            input_refs: vec![String::from(LEARNING_RECEIPTS_REF)],
            output_refs: vec![String::from(REPLAY_BUNDLE_REF)],
            emitted_ids: vec![
                String::from(
                    "sample.route.receipt.compiled_agent.learning.openagents_negated_wallet_receipt_v1",
                ),
                String::from(
                    "sample.grounded_answer.receipt.compiled_agent.learning.openagents_wallet_recent_earnings_receipt_v1",
                ),
                String::from("bundle.compiled_agent_replay_bundle_v1"),
            ],
            next_consumer: String::from("compiled_agent.role.bounded_module_training.v1"),
            detail: String::from(
                "The retained replay-generation reference path starts from the governed learning ledger and emits replay rows that already feed the current route and grounded training jobs.",
            ),
        },
        CompiledAgentDecentralizedRoleReceipt {
            role: CompiledAgentDecentralizedRoleKind::RankingLabeling,
            receipt_id: String::from("compiled_agent.role_receipt.ranking_labeling.v1"),
            evidence_class: contract.evidence_class,
            status: CompiledAgentRoleReceiptStatus::AcceptedAfterHumanReview,
            input_refs: vec![
                String::from(LEARNING_RECEIPTS_REF),
                String::from(REPLAY_BUNDLE_REF),
            ],
            output_refs: vec![
                String::from(LEARNING_RECEIPTS_REF),
                String::from(REPLAY_BUNDLE_REF),
            ],
            emitted_ids: vec![
                String::from("openagents_negated_wallet_receipt_v1"),
                String::from("openagents_ambiguous_provider_wallet_receipt_v1"),
                String::from("openagents_wallet_earnings_phrase_heldout_receipt_v1"),
            ],
            next_consumer: String::from("compiled_agent.role.replay_generation.v1"),
            detail: String::from(
                "The retained ranking-and-labeling reference path keeps corpus placement and acceptance decisions explicitly human-reviewed before they enter replay or held-out truth.",
            ),
        },
        CompiledAgentDecentralizedRoleReceipt {
            role: CompiledAgentDecentralizedRoleKind::ValidatorScoring,
            receipt_id: String::from("compiled_agent.role_receipt.validator_scoring.v1"),
            evidence_class: contract.evidence_class,
            status: CompiledAgentRoleReceiptStatus::AcceptedAfterValidatorGate,
            input_refs: vec![
                String::from(ROUTE_MODEL_REF),
                String::from(GROUNDED_MODEL_REF),
                String::from(ROUTE_REPORT_REF),
                String::from(GROUNDED_REPORT_REF),
                String::from(ROUTE_STRONGER_MODEL_REF),
                String::from(GROUNDED_STRONGER_MODEL_REF),
                String::from(ROUTE_STRONGER_REPORT_REF),
                String::from(GROUNDED_STRONGER_REPORT_REF),
                String::from(STRONGER_CANDIDATE_FAMILY_REPORT_REF),
                String::from(XTRAIN_RECEIPT_REF),
                String::from(CONFIDENCE_POLICY_REF),
                String::from(SHADOW_DISAGREEMENTS_REF),
            ],
            output_refs: vec![
                String::from(XTRAIN_RECEIPT_REF),
                String::from(PROMOTED_CONTRACT_REF),
            ],
            emitted_ids: vec![
                String::from("compiled_agent.route.multinomial_nb_v1.validation"),
                String::from("compiled_agent.grounded_answer.multinomial_nb_v1.validation"),
                String::from("compiled_agent.route.tfidf_centroid_v1.validation"),
                String::from("compiled_agent.grounded_answer.tfidf_centroid_v1.validation"),
                String::from("compiled_agent.stronger_candidate_family_report.v1"),
                String::from("compiled_agent.xtrain.cycle.v1"),
            ],
            next_consumer: String::from("compiled_agent_runtime_authority"),
            detail: String::from(
                "The retained validator-scoring reference path is the first governed promotion surface. It consumes incumbent-versus-stronger comparisons, confidence policy, and shadow disagreements, then ends in the XTRAIN receipt and the promoted-artifact contract instead of silent runtime mutation.",
            ),
        },
        CompiledAgentDecentralizedRoleReceipt {
            role: CompiledAgentDecentralizedRoleKind::BoundedModuleTraining,
            receipt_id: String::from("compiled_agent.role_receipt.bounded_module_training.v1"),
            evidence_class: contract.evidence_class,
            status: CompiledAgentRoleReceiptStatus::QueuedForValidatorScoring,
            input_refs: vec![
                String::from(DEFAULT_ROW_REF),
                String::from(LEARNING_RECEIPTS_REF),
                String::from(REPLAY_BUNDLE_REF),
            ],
            output_refs: vec![
                String::from(ROUTE_MODEL_REF),
                String::from(GROUNDED_MODEL_REF),
                String::from(ROUTE_STRONGER_MODEL_REF),
                String::from(GROUNDED_STRONGER_MODEL_REF),
            ],
            emitted_ids: vec![
                String::from("compiled_agent.route.multinomial_nb_v1"),
                String::from("compiled_agent.grounded_answer.multinomial_nb_v1"),
                String::from("compiled_agent.route.tfidf_centroid_v1"),
                String::from("compiled_agent.grounded_answer.tfidf_centroid_v1"),
            ],
            next_consumer: String::from("compiled_agent.role.validator_scoring.v1"),
            detail: String::from(
                "The retained bounded-module-training reference path produces incumbent and stronger bounded candidate artifacts only. Validator scoring still decides whether those candidates can move runtime authority.",
            ),
        },
    ];

    let mut bundle = CompiledAgentDecentralizedRoleReceipts {
        schema_version: String::from(COMPILED_AGENT_DECENTRALIZED_ROLES_RECEIPTS_SCHEMA_VERSION),
        contract_digest: contract.contract_digest.clone(),
        receipts,
        summary: String::new(),
        receipts_digest: String::new(),
    };
    bundle.summary = format!(
        "Compiled-agent decentralized role receipts retain {} governed pre-network roles tied back to contract {}.",
        bundle.receipts.len(),
        bundle.contract_digest
    );
    bundle.receipts_digest = bundle.stable_digest();
    bundle.validate(&contract)?;
    Ok(bundle)
}

pub fn canonical_compiled_agent_decentralized_role_dry_run_report()
-> Result<CompiledAgentDecentralizedRoleDryRunReport, CompiledAgentDecentralizedRolesError> {
    let contract = canonical_compiled_agent_decentralized_roles_contract()?;
    let receipts = canonical_compiled_agent_decentralized_role_receipts()?;
    let learning_ledger = canonical_compiled_agent_learning_receipt_ledger()?;
    let replay_bundle = canonical_compiled_agent_replay_bundle()?;
    let xtrain_receipt = canonical_compiled_agent_xtrain_cycle_receipt()?;
    let promoted_contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let confidence_policy = canonical_compiled_agent_confidence_policy()?;
    let shadow_disagreements = canonical_compiled_agent_shadow_disagreement_receipts()?;
    let stronger_family_report = canonical_compiled_agent_stronger_candidate_family_report()?;
    let runtime_fixture_refs = canonical_runtime_fixture_refs();
    let runtime_learning_receipt_ids = canonical_runtime_learning_receipt_ids()?;
    let validator_discipline_unchanged = validator_discipline_is_unchanged(
        &receipts,
        &xtrain_receipt,
        &stronger_family_report,
        &confidence_policy,
        &shadow_disagreements,
    );
    let rollback_discipline_unchanged = rollback_discipline_is_unchanged(
        &promoted_contract,
        &confidence_policy,
        &shadow_disagreements,
    );

    let role_runs = contract
        .roles
        .iter()
        .enumerate()
        .map(|(index, role)| {
            let receipt = receipts
                .receipts
                .iter()
                .find(|candidate| candidate.role == role.role)
                .expect("governed dry-run report must have one retained receipt per role");
            CompiledAgentDecentralizedDryRunStep {
                order: (index + 1) as u32,
                role: role.role,
                role_id: role.role_id.clone(),
                receipt_id: receipt.receipt_id.clone(),
                status: receipt.status,
                review_boundary: role.review_boundary,
                validator_gate: role.validator_gate.clone(),
                consumed_artifacts: role.input_manifest.required_artifacts.clone(),
                produced_artifacts: role.output_manifest.required_artifacts.clone(),
                runtime_fixture_refs: runtime_fixture_refs.clone(),
                runtime_learning_receipt_ids: runtime_learning_receipt_ids.clone(),
                emitted_ids: receipt.emitted_ids.clone(),
                next_consumer: receipt.next_consumer.clone(),
                lineage_intact: true,
                validator_discipline_preserved: validator_discipline_unchanged,
                rollback_discipline_preserved: rollback_discipline_unchanged,
                detail: dry_run_step_detail(role.role),
            }
        })
        .collect::<Vec<_>>();

    let mut report = CompiledAgentDecentralizedRoleDryRunReport {
        schema_version: String::from(COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_SCHEMA_VERSION),
        dry_run_id: String::from(COMPILED_AGENT_DECENTRALIZED_ROLE_DRY_RUN_ID),
        evidence_class: contract.evidence_class,
        contract_digest: contract.contract_digest.clone(),
        receipts_digest: receipts.receipts_digest.clone(),
        learning_ledger_digest: learning_ledger.ledger_digest,
        replay_bundle_digest: replay_bundle.bundle_digest,
        xtrain_receipt_digest: xtrain_receipt.receipt_digest,
        promoted_contract_digest: promoted_contract.contract_digest,
        confidence_policy_digest: confidence_policy.policy_digest,
        shadow_disagreement_receipts_digest: shadow_disagreements.receipts_digest,
        stronger_candidate_family_report_digest: stronger_family_report.report_digest,
        runtime_fixture_refs,
        runtime_learning_receipt_ids,
        role_runs,
        validator_discipline_unchanged,
        rollback_discipline_unchanged,
        claim_boundary: String::from(
            "This retained dry run covers only the admitted compiled-agent family with governed receipts, explicit validator gates, and explicit rollback authority. It does not claim broad decentralized autonomy or validator-free promotion.",
        ),
        summary: String::from(
            "Compiled-agent decentralized roles now rerun as a boring dry run over the stricter bounded corpus and runtime-ingested receipts while preserving validator and rollback discipline.",
        ),
        report_digest: String::new(),
    };
    report.report_digest = report.stable_digest();
    report.validate(&contract, &receipts)?;
    Ok(report)
}

pub fn write_compiled_agent_decentralized_roles_contract(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentDecentralizedRolesContract, CompiledAgentDecentralizedRolesError> {
    write_json(
        output_path,
        &canonical_compiled_agent_decentralized_roles_contract()?,
    )
}

pub fn write_compiled_agent_decentralized_role_receipts(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentDecentralizedRoleReceipts, CompiledAgentDecentralizedRolesError> {
    write_json(
        output_path,
        &canonical_compiled_agent_decentralized_role_receipts()?,
    )
}

pub fn write_compiled_agent_decentralized_role_dry_run(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentDecentralizedRoleDryRunReport, CompiledAgentDecentralizedRolesError> {
    write_json(
        output_path,
        &canonical_compiled_agent_decentralized_role_dry_run_report()?,
    )
}

pub fn verify_compiled_agent_decentralized_role_fixtures()
-> Result<(), CompiledAgentDecentralizedRolesError> {
    let expected_contract = canonical_compiled_agent_decentralized_roles_contract()?;
    let contract_path = compiled_agent_decentralized_roles_contract_fixture_path();
    let committed_contract: CompiledAgentDecentralizedRolesContract =
        serde_json::from_slice(&fs::read(&contract_path).map_err(|error| {
            CompiledAgentDecentralizedRolesError::Read {
                path: contract_path.display().to_string(),
                error,
            }
        })?)?;
    let committed_contract_json = serde_json::to_string_pretty(&committed_contract)?;
    let expected_contract_json = serde_json::to_string_pretty(&expected_contract)?;
    if committed_contract_json != expected_contract_json {
        return Err(CompiledAgentDecentralizedRolesError::FixtureDrift {
            path: contract_path.display().to_string(),
        });
    }

    let expected_receipts = canonical_compiled_agent_decentralized_role_receipts()?;
    let receipts_path = compiled_agent_decentralized_role_receipts_fixture_path();
    let committed_receipts: CompiledAgentDecentralizedRoleReceipts =
        serde_json::from_slice(&fs::read(&receipts_path).map_err(|error| {
            CompiledAgentDecentralizedRolesError::Read {
                path: receipts_path.display().to_string(),
                error,
            }
        })?)?;
    let committed_receipts_json = serde_json::to_string_pretty(&committed_receipts)?;
    let expected_receipts_json = serde_json::to_string_pretty(&expected_receipts)?;
    if committed_receipts_json != expected_receipts_json {
        return Err(CompiledAgentDecentralizedRolesError::FixtureDrift {
            path: receipts_path.display().to_string(),
        });
    }

    let expected_dry_run = canonical_compiled_agent_decentralized_role_dry_run_report()?;
    let dry_run_path = compiled_agent_decentralized_role_dry_run_fixture_path();
    let committed_dry_run: CompiledAgentDecentralizedRoleDryRunReport =
        serde_json::from_slice(&fs::read(&dry_run_path).map_err(|error| {
            CompiledAgentDecentralizedRolesError::Read {
                path: dry_run_path.display().to_string(),
                error,
            }
        })?)?;
    let committed_dry_run_json = serde_json::to_string_pretty(&committed_dry_run)?;
    let expected_dry_run_json = serde_json::to_string_pretty(&expected_dry_run)?;
    if committed_dry_run_json != expected_dry_run_json {
        return Err(CompiledAgentDecentralizedRolesError::FixtureDrift {
            path: dry_run_path.display().to_string(),
        });
    }
    Ok(())
}

pub fn compiled_agent_decentralized_role_snapshot(
    role: CompiledAgentDecentralizedRoleKind,
) -> Result<
    (
        CompiledAgentDecentralizedRoleDefinition,
        CompiledAgentDecentralizedRoleReceipt,
    ),
    CompiledAgentDecentralizedRolesError,
> {
    let contract = canonical_compiled_agent_decentralized_roles_contract()?;
    let receipts = canonical_compiled_agent_decentralized_role_receipts()?;
    let definition = contract
        .roles
        .iter()
        .find(|candidate| candidate.role == role)
        .cloned()
        .ok_or_else(|| CompiledAgentDecentralizedRolesError::InvalidContract {
            detail: format!("role `{:?}` missing from contract", role),
        })?;
    let receipt = receipts
        .receipts
        .iter()
        .find(|candidate| candidate.role == role)
        .cloned()
        .ok_or_else(|| CompiledAgentDecentralizedRolesError::InvalidReceipts {
            detail: format!("role `{:?}` missing from receipts", role),
        })?;
    Ok((definition, receipt))
}

fn canonical_source_artifacts()
-> Result<Vec<CompiledAgentRoleArtifactRef>, CompiledAgentDecentralizedRolesError> {
    Ok(vec![
        load_artifact_ref(
            LEARNING_RECEIPTS_REF,
            Some("ledger_digest"),
            None,
            "Canonical compiled-agent learning ledger retained from normalized runtime receipts.",
        )?,
        load_artifact_ref(
            REPLAY_BUNDLE_REF,
            Some("bundle_digest"),
            None,
            "Canonical replay bundle emitted from the retained compiled-agent learning ledger.",
        )?,
        load_artifact_ref(
            DEFAULT_ROW_REF,
            None,
            Some("row_id"),
            "Locked default learned row for the first compiled-agent target lane.",
        )?,
        load_artifact_ref(
            ROUTE_MODEL_REF,
            Some("artifact_digest"),
            Some("artifact_id"),
            "Retained replay-trained route artifact consumed by validator scoring and runtime authority.",
        )?,
        load_artifact_ref(
            GROUNDED_MODEL_REF,
            Some("artifact_digest"),
            Some("artifact_id"),
            "Retained replay-trained grounded-answer artifact consumed by validator scoring and runtime authority.",
        )?,
        load_artifact_ref(
            ROUTE_REPORT_REF,
            Some("report_digest"),
            None,
            "Independent route candidate module-eval surface for validator scoring.",
        )?,
        load_artifact_ref(
            GROUNDED_REPORT_REF,
            Some("report_digest"),
            None,
            "Independent grounded-answer candidate module-eval surface for validator scoring.",
        )?,
        load_artifact_ref(
            ROUTE_STRONGER_MODEL_REF,
            Some("artifact_digest"),
            Some("artifact_id"),
            "Retained stronger route candidate family artifact kept behind the same bounded route contract.",
        )?,
        load_artifact_ref(
            GROUNDED_STRONGER_MODEL_REF,
            Some("artifact_digest"),
            Some("artifact_id"),
            "Retained stronger grounded-answer candidate family artifact kept behind the same bounded synthesis contract.",
        )?,
        load_artifact_ref(
            ROUTE_STRONGER_REPORT_REF,
            Some("report_digest"),
            None,
            "Independent route module-eval report for the stronger bounded candidate family.",
        )?,
        load_artifact_ref(
            GROUNDED_STRONGER_REPORT_REF,
            Some("report_digest"),
            None,
            "Independent grounded-answer module-eval report for the stronger bounded candidate family.",
        )?,
        load_artifact_ref(
            STRONGER_CANDIDATE_FAMILY_REPORT_REF,
            Some("report_digest"),
            Some("report_id"),
            "Retained incumbent-versus-stronger bounded family comparison report for route and grounded-answer modules.",
        )?,
        load_artifact_ref(
            XTRAIN_RECEIPT_REF,
            Some("receipt_digest"),
            Some("cycle_id"),
            "Canonical bounded XTRAIN cycle receipt for the current route and grounded candidates.",
        )?,
        load_artifact_ref(
            PROMOTED_CONTRACT_REF,
            Some("contract_digest"),
            Some("schema_version"),
            "Runtime-consumable promoted-artifact contract retained after validator gating.",
        )?,
        load_artifact_ref(
            CONFIDENCE_POLICY_REF,
            Some("policy_digest"),
            Some("policy_id"),
            "Retained confidence policy for promoted-versus-candidate calibration and rollback review thresholds.",
        )?,
        load_artifact_ref(
            SHADOW_DISAGREEMENTS_REF,
            Some("receipts_digest"),
            Some("ledger_id"),
            "Retained promoted-versus-candidate disagreement receipts across held-out and runtime rows.",
        )?,
    ])
}

fn canonical_runtime_fixture_refs() -> Vec<String> {
    let mut refs = canonical_compiled_agent_runtime_source_receipts()
        .into_iter()
        .map(|(fixture_name, _)| format!("{COMPILED_AGENT_RUNTIME_FIXTURE_DIR}/{fixture_name}"))
        .collect::<Vec<_>>();
    refs.sort();
    refs
}

fn canonical_runtime_learning_receipt_ids()
-> Result<Vec<String>, CompiledAgentDecentralizedRolesError> {
    let mut receipt_ids = canonical_compiled_agent_learning_receipt_ledger()?
        .receipts
        .into_iter()
        .filter(|receipt| {
            receipt
                .source_fixture_ref
                .starts_with(&format!("{COMPILED_AGENT_RUNTIME_FIXTURE_DIR}/"))
        })
        .map(|receipt| receipt.receipt_id)
        .collect::<Vec<_>>();
    receipt_ids.sort();
    Ok(receipt_ids)
}

fn validator_discipline_is_unchanged(
    receipts: &CompiledAgentDecentralizedRoleReceipts,
    xtrain_receipt: &crate::CompiledAgentXtrainCycleReceipt,
    stronger_family_report: &crate::CompiledAgentStrongerCandidateFamilyReport,
    confidence_policy: &crate::CompiledAgentConfidencePolicy,
    shadow_disagreements: &crate::CompiledAgentShadowDisagreementReceipts,
) -> bool {
    let role_statuses = receipts
        .receipts
        .iter()
        .map(|receipt| (receipt.role, receipt.status))
        .collect::<std::collections::BTreeMap<_, _>>();
    let expected_statuses = [
        (
            CompiledAgentDecentralizedRoleKind::ReplayGeneration,
            CompiledAgentRoleReceiptStatus::AcceptedAfterHumanReview,
        ),
        (
            CompiledAgentDecentralizedRoleKind::RankingLabeling,
            CompiledAgentRoleReceiptStatus::AcceptedAfterHumanReview,
        ),
        (
            CompiledAgentDecentralizedRoleKind::ValidatorScoring,
            CompiledAgentRoleReceiptStatus::AcceptedAfterValidatorGate,
        ),
        (
            CompiledAgentDecentralizedRoleKind::BoundedModuleTraining,
            CompiledAgentRoleReceiptStatus::QueuedForValidatorScoring,
        ),
    ]
    .into_iter()
    .all(|(role, status)| role_statuses.get(&role) == Some(&status));

    let xtrain_gate_intact = xtrain_receipt.route_outcome.decision
        == CompiledAgentPromotionDecision::Promote
        && xtrain_receipt.grounded_answer_outcome.decision
            == CompiledAgentPromotionDecision::Promote;
    let stronger_gate_intact = stronger_family_report.comparisons.iter().all(|comparison| {
        comparison.decision == CompiledAgentCandidateFamilyDecision::KeepIncumbent
    });
    let human_review_gates_intact = confidence_policy.policies.iter().all(|policy| {
        policy
            .thresholds
            .human_review_on_low_confidence_disagreement
            && policy.thresholds.human_review_on_ambiguous_regression
    });

    expected_statuses
        && xtrain_gate_intact
        && stronger_gate_intact
        && human_review_gates_intact
        && shadow_disagreements.human_review_count > 0
}

fn rollback_discipline_is_unchanged(
    promoted_contract: &crate::CompiledAgentPromotedArtifactContract,
    confidence_policy: &crate::CompiledAgentConfidencePolicy,
    shadow_disagreements: &crate::CompiledAgentShadowDisagreementReceipts,
) -> bool {
    let route_promoted_has_rollback = promoted_contract
        .promoted_entry(CompiledAgentModuleKind::Route)
        .and_then(|entry| entry.rollback_artifact_id.as_ref())
        .is_some();
    let route_policy = confidence_policy
        .policies
        .iter()
        .find(|policy| policy.module == CompiledAgentModuleKind::Route);
    let grounded_policy = confidence_policy
        .policies
        .iter()
        .find(|policy| policy.module == CompiledAgentModuleKind::GroundedAnswer);

    route_promoted_has_rollback
        && route_policy.is_some_and(|policy| {
            policy.thresholds.rollback_on_heldout_regression_count == 1
                && policy.thresholds.rollback_on_runtime_regression_count == 1
        })
        && grounded_policy.is_some_and(|policy| {
            policy.thresholds.rollback_on_heldout_regression_count == 1
                && policy.thresholds.rollback_on_runtime_regression_count == 1
        })
        && shadow_disagreements.rollback_ready_count == 0
}

fn dry_run_step_detail(role: CompiledAgentDecentralizedRoleKind) -> String {
    match role {
        CompiledAgentDecentralizedRoleKind::ReplayGeneration => String::from(
            "Replay generation reran against the stricter learning ledger, including runtime-ingested wallet and disagreement rows, without changing receipt lineage.",
        ),
        CompiledAgentDecentralizedRoleKind::RankingLabeling => String::from(
            "Ranking and labeling retained human-reviewed corpus placement for the same narrow bounded family, including runtime disagreement and correction rows.",
        ),
        CompiledAgentDecentralizedRoleKind::ValidatorScoring => String::from(
            "Validator scoring reran against incumbent-versus-stronger candidates, confidence policy, and shadow disagreement receipts while preserving the validator gate.",
        ),
        CompiledAgentDecentralizedRoleKind::BoundedModuleTraining => String::from(
            "Bounded module training reran on the locked default row and stricter replay bundle, producing incumbent and stronger candidate artifacts that still queue for validator scoring.",
        ),
    }
}

fn artifact_ref_from_set(
    artifact_set: &[CompiledAgentRoleArtifactRef],
    artifact_ref: &str,
) -> Result<CompiledAgentRoleArtifactRef, CompiledAgentDecentralizedRolesError> {
    artifact_set
        .iter()
        .find(|artifact| artifact.artifact_ref == artifact_ref)
        .cloned()
        .ok_or_else(|| CompiledAgentDecentralizedRolesError::InvalidContract {
            detail: format!("source artifact `{artifact_ref}` missing"),
        })
}

fn load_artifact_ref(
    artifact_ref: &str,
    digest_field: Option<&str>,
    identity_field: Option<&str>,
    detail: &str,
) -> Result<CompiledAgentRoleArtifactRef, CompiledAgentDecentralizedRolesError> {
    let path = repo_relative_path(artifact_ref);
    let value: Value = serde_json::from_slice(&fs::read(&path).map_err(|error| {
        CompiledAgentDecentralizedRolesError::Read {
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
        .map_err(CompiledAgentDecentralizedRolesError::Json)?;
    let artifact_digest = digest_field
        .and_then(|field| value.get(field))
        .map(value_to_string);
    if digest_field.is_some() && artifact_digest.as_deref().unwrap_or_default().is_empty() {
        return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
            detail: format!("artifact `{artifact_ref}` lost required digest field"),
        });
    }
    let identity_value = identity_field
        .and_then(|field| value.get(field))
        .map(value_to_string);
    if identity_field.is_some() && identity_value.as_deref().unwrap_or_default().is_empty() {
        return Err(CompiledAgentDecentralizedRolesError::InvalidContract {
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

fn ensure_matching_evidence_class(
    expected: CompiledAgentEvidenceClass,
    actual: CompiledAgentEvidenceClass,
    context: String,
) -> Result<(), CompiledAgentDecentralizedRolesError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CompiledAgentDecentralizedRolesError::MixedEvidenceClass {
            context,
            expected,
            actual,
        })
    }
}

fn local_reference_path(
    role_selector: &str,
    retained_output_refs: Vec<String>,
    detail: &str,
) -> CompiledAgentRoleReferencePath {
    CompiledAgentRoleReferencePath {
        command: format!(
            "cargo run -q -p psionic-train --bin compiled_agent_decentralized_roles -- --role {role_selector}"
        ),
        bin_path: String::from(COMPILED_AGENT_DECENTRALIZED_ROLES_BIN_PATH),
        role_selector: String::from(role_selector),
        retained_output_refs,
        detail: String::from(detail),
    }
}

fn write_json<T: Serialize>(
    output_path: impl AsRef<Path>,
    value: &T,
) -> Result<T, CompiledAgentDecentralizedRolesError>
where
    T: Clone,
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CompiledAgentDecentralizedRolesError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let json = serde_json::to_string_pretty(value)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentDecentralizedRolesError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(value.clone())
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(string) => string.clone(),
        _ => value.to_string(),
    }
}

fn stable_digest(prefix: &[u8], value: &impl Serialize) -> String {
    let canonical = serde_json::to_vec(value)
        .expect("stable digest serialization must succeed for role contracts");
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(&canonical);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        CompiledAgentDecentralizedRoleKind, canonical_compiled_agent_decentralized_role_dry_run_report,
        canonical_compiled_agent_decentralized_role_receipts,
        canonical_compiled_agent_decentralized_roles_contract,
        verify_compiled_agent_decentralized_role_fixtures,
    };
    use psionic_eval::CompiledAgentEvidenceClass;

    #[test]
    fn compiled_agent_decentralized_roles_contract_is_valid()
    -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_decentralized_roles_contract()?;
        contract.validate()?;
        assert_eq!(
            contract.evidence_class,
            CompiledAgentEvidenceClass::LearnedLane
        );
        assert_eq!(contract.roles.len(), 4);
        assert!(
            contract
                .roles
                .iter()
                .any(|role| { role.role == CompiledAgentDecentralizedRoleKind::ValidatorScoring })
        );
        Ok(())
    }

    #[test]
    fn compiled_agent_decentralized_role_receipts_are_valid()
    -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_decentralized_roles_contract()?;
        let receipts = canonical_compiled_agent_decentralized_role_receipts()?;
        receipts.validate(&contract)?;
        assert_eq!(receipts.receipts.len(), 4);
        assert!(
            receipts
                .receipts
                .iter()
                .all(|receipt| receipt.evidence_class == CompiledAgentEvidenceClass::LearnedLane)
        );
        Ok(())
    }

    #[test]
    fn compiled_agent_decentralized_role_dry_run_report_is_valid()
    -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_decentralized_roles_contract()?;
        let receipts = canonical_compiled_agent_decentralized_role_receipts()?;
        let report = canonical_compiled_agent_decentralized_role_dry_run_report()?;
        report.validate(&contract, &receipts)?;
        assert!(report.validator_discipline_unchanged);
        assert!(report.rollback_discipline_unchanged);
        assert_eq!(report.role_runs.len(), 4);
        Ok(())
    }

    #[test]
    fn compiled_agent_decentralized_role_fixtures_match_the_canonical_generator()
    -> Result<(), Box<dyn std::error::Error>> {
        verify_compiled_agent_decentralized_role_fixtures()?;
        Ok(())
    }
}
