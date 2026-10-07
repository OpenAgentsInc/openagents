use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use psionic_eval::{
    build_compiled_agent_module_eval_report, canonical_compiled_agent_default_row_contract,
    compiled_agent_baseline_revision_set, CompiledAgentDefaultLearnedRowContract,
    CompiledAgentEvidenceClass, CompiledAgentGroundedAnswerModelArtifact,
    CompiledAgentModuleEvalReport, CompiledAgentModuleKind, CompiledAgentModuleRevisionSet,
    CompiledAgentRouteModelArtifact,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    canonical_compiled_agent_route_model_artifact, canonical_compiled_agent_xtrain_cycle_receipt,
    repo_relative_path, CompiledAgentPromotionDecision, CompiledAgentXtrainError,
};

pub const COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_FIXTURE_PATH: &str =
    "fixtures/compiled_agent/compiled_agent_promoted_artifact_contract_v1.json";

const COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_SCHEMA_VERSION: &str =
    "psionic.compiled_agent.promoted_artifact_contract.v1";
const COMPILED_AGENT_COMPATIBILITY_VERSION: &str = "openagents.compiled_agent.first_graph.v1";
const PROMOTED_AT_UTC: &str = "2026-03-29T00:00:00Z";
const ROUTE_MODEL_FIXTURE_REF: &str = "fixtures/compiled_agent/compiled_agent_route_model_v1.json";
const ROUTE_CANDIDATE_REPORT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_route_candidate_module_eval_report_v1.json";
const GROUNDED_MODEL_FIXTURE_REF: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_answer_model_v1.json";
const GROUNDED_CANDIDATE_REPORT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_grounded_candidate_module_eval_report_v1.json";
const BASELINE_REPORT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_module_eval_report_v1.json";
const XTRAIN_CYCLE_RECEIPT_REF: &str =
    "fixtures/compiled_agent/compiled_agent_xtrain_cycle_receipt_v1.json";

#[derive(Debug, Error)]
pub enum CompiledAgentArtifactContractError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
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
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Xtrain(#[from] CompiledAgentXtrainError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompiledAgentArtifactLifecycleState {
    Promoted,
    Candidate,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "payload_kind", rename_all = "snake_case")]
pub enum CompiledAgentArtifactPayload {
    RevisionSet {
        revision: CompiledAgentModuleRevisionSet,
    },
    RouteModel {
        artifact: CompiledAgentRouteModelArtifact,
    },
    /// A trained frozen-backbone coordinator head (Khala M6 / TRINITY). The
    /// payload is the digest-pinned head identity (config + flat-parameter
    /// digest) rather than the backbone weights, matching the
    /// `AdapterArtifactKind::CoordinatorHead` packaging contract. This variant
    /// is additive: the existing `RevisionSet` / `RouteModel` payloads keyed to
    /// `CompiledAgentModuleKind` modules are unchanged.
    CoordinatorHead {
        artifact: crate::CoordinatorHeadCandidateArtifact,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentArtifactValidatorLineage {
    pub validator_report_ref: String,
    pub validator_report_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xtrain_cycle_receipt_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xtrain_cycle_receipt_digest: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentArtifactContractEntry {
    pub module: CompiledAgentModuleKind,
    pub module_name: String,
    pub signature_name: String,
    pub implementation_family: String,
    pub implementation_label: String,
    pub version: String,
    pub lifecycle_state: CompiledAgentArtifactLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_label: Option<String>,
    pub compatibility_version: String,
    pub confidence_floor: f32,
    pub artifact_id: String,
    pub artifact_digest: String,
    pub row_id: String,
    pub default_row: CompiledAgentDefaultLearnedRowContract,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub validator_lineage: CompiledAgentArtifactValidatorLineage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub predecessor_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rollback_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promoted_at_utc: Option<String>,
    pub payload: CompiledAgentArtifactPayload,
    pub detail: String,
}

impl CompiledAgentArtifactContractEntry {
    #[must_use]
    pub fn manifest_id(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.module_name, self.implementation_family, self.implementation_label, self.version
        )
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledAgentPromotedArtifactContract {
    pub schema_version: String,
    pub ledger_id: String,
    pub row_id: String,
    pub evidence_class: CompiledAgentEvidenceClass,
    pub promoted_entry_count: u32,
    pub candidate_entry_count: u32,
    pub entries_by_module: BTreeMap<String, u32>,
    pub artifacts: Vec<CompiledAgentArtifactContractEntry>,
    pub summary: String,
    pub contract_digest: String,
}

impl CompiledAgentPromotedArtifactContract {
    #[must_use]
    pub fn promoted_entry(
        &self,
        module: CompiledAgentModuleKind,
    ) -> Option<&CompiledAgentArtifactContractEntry> {
        self.artifacts.iter().find(|entry| {
            entry.module == module
                && entry.lifecycle_state == CompiledAgentArtifactLifecycleState::Promoted
        })
    }

    #[must_use]
    pub fn candidate_entry(
        &self,
        module: CompiledAgentModuleKind,
        candidate_label: &str,
    ) -> Option<&CompiledAgentArtifactContractEntry> {
        self.artifacts.iter().find(|entry| {
            entry.module == module
                && entry.lifecycle_state == CompiledAgentArtifactLifecycleState::Candidate
                && entry.candidate_label.as_deref() == Some(candidate_label)
        })
    }
}

#[must_use]
pub fn compiled_agent_promoted_artifact_contract_fixture_path() -> PathBuf {
    repo_relative_path(COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_FIXTURE_PATH)
}

pub fn canonical_compiled_agent_promoted_artifact_contract(
) -> Result<CompiledAgentPromotedArtifactContract, CompiledAgentArtifactContractError> {
    let default_row = canonical_compiled_agent_default_row_contract();
    let route_model = canonical_compiled_agent_route_model_artifact()?;
    let baseline_revision = compiled_agent_baseline_revision_set();
    let grounded_model_artifact = read_grounded_model_artifact_fixture()?;
    let grounded_candidate_revision =
        grounded_candidate_revision_from_artifact(grounded_model_artifact.clone());
    let baseline_report = build_compiled_agent_module_eval_report(&baseline_revision);
    let route_candidate_report = build_compiled_agent_module_eval_report(
        &route_candidate_route_revision(route_model.clone()),
    );
    let grounded_candidate_report =
        build_compiled_agent_module_eval_report(&grounded_candidate_revision);
    let xtrain_cycle = canonical_compiled_agent_xtrain_cycle_receipt()?;
    let evidence_class = xtrain_cycle.evidence_class;
    ensure_matching_evidence_class(
        evidence_class,
        baseline_report.evidence_class,
        "compiled_agent_promoted_artifact_contract.baseline_report",
    )?;
    ensure_matching_evidence_class(
        evidence_class,
        route_candidate_report.evidence_class,
        "compiled_agent_promoted_artifact_contract.route_candidate_report",
    )?;
    ensure_matching_evidence_class(
        evidence_class,
        grounded_candidate_report.evidence_class,
        "compiled_agent_promoted_artifact_contract.grounded_candidate_report",
    )?;

    let baseline_lineage = validator_lineage(
        BASELINE_REPORT_REF,
        &baseline_report,
        Some(XTRAIN_CYCLE_RECEIPT_REF),
        Some(&xtrain_cycle.receipt_digest),
    );
    let route_lineage = validator_lineage(
        ROUTE_CANDIDATE_REPORT_REF,
        &route_candidate_report,
        Some(XTRAIN_CYCLE_RECEIPT_REF),
        Some(&xtrain_cycle.receipt_digest),
    );
    let grounded_lineage = validator_lineage(
        GROUNDED_CANDIDATE_REPORT_REF,
        &grounded_candidate_report,
        Some(XTRAIN_CYCLE_RECEIPT_REF),
        Some(&xtrain_cycle.receipt_digest),
    );

    let route_fallback_artifact_id = String::from("compiled_agent.baseline.rule_v1.route");
    let (promoted_route, route_candidate_entries) = match xtrain_cycle.route_outcome.decision {
        CompiledAgentPromotionDecision::Promote => {
            let promoted_route = CompiledAgentArtifactContractEntry {
                module: CompiledAgentModuleKind::Route,
                module_name: String::from("intent_route"),
                signature_name: String::from("intent_route"),
                implementation_family: String::from("psionic_route_model"),
                implementation_label: route_model.artifact_id.clone(),
                version: String::from("2026-03-29"),
                lifecycle_state: CompiledAgentArtifactLifecycleState::Promoted,
                candidate_label: None,
                compatibility_version: String::from(COMPILED_AGENT_COMPATIBILITY_VERSION),
                confidence_floor: 0.8,
                artifact_id: route_model.artifact_id.clone(),
                artifact_digest: route_model.artifact_digest.clone(),
                row_id: default_row.row_id.clone(),
                default_row: default_row.clone(),
                evidence_class,
                validator_lineage: route_lineage.clone(),
                predecessor_artifact_id: Some(route_fallback_artifact_id.clone()),
                rollback_artifact_id: Some(route_fallback_artifact_id.clone()),
                promoted_at_utc: Some(String::from(PROMOTED_AT_UTC)),
                payload: CompiledAgentArtifactPayload::RouteModel {
                    artifact: route_model.clone(),
                },
                detail: format!(
                    "Promoted route artifact sourced from {} and validator-scored through the bounded compiled-agent XTRAIN cycle.",
                    ROUTE_MODEL_FIXTURE_REF
                ),
            };
            let rollback_route = revision_entry(
                CompiledAgentModuleKind::Route,
                "intent_route",
                "intent_route",
                "psionic_rule_revision",
                "last_known_good",
                "2026-03-28",
                CompiledAgentArtifactLifecycleState::Candidate,
                Some("last_known_good"),
                0.8,
                route_fallback_artifact_id,
                &baseline_revision,
                &default_row,
                evidence_class,
                baseline_lineage.clone(),
                None,
                None,
                "Rollback route artifact that preserves the last-known-good baseline revision for clean candidate-authority rollback.",
            );
            (promoted_route, vec![rollback_route])
        }
        CompiledAgentPromotionDecision::Hold => {
            let promoted_route = revision_entry(
                CompiledAgentModuleKind::Route,
                "intent_route",
                "intent_route",
                "psionic_rule_revision",
                "promoted",
                "2026-03-28",
                CompiledAgentArtifactLifecycleState::Promoted,
                None,
                0.8,
                route_fallback_artifact_id.clone(),
                &baseline_revision,
                &default_row,
                evidence_class,
                baseline_lineage.clone(),
                None,
                Some(String::from(PROMOTED_AT_UTC)),
                "Promoted route artifact remains the last-known-good baseline revision because the widened held-out split held the learned route candidate.",
            );
            let route_candidate = CompiledAgentArtifactContractEntry {
                module: CompiledAgentModuleKind::Route,
                module_name: String::from("intent_route"),
                signature_name: String::from("intent_route"),
                implementation_family: String::from("psionic_route_model"),
                implementation_label: route_model.artifact_id.clone(),
                version: String::from("2026-03-29"),
                lifecycle_state: CompiledAgentArtifactLifecycleState::Candidate,
                candidate_label: Some(String::from("psionic_candidate")),
                compatibility_version: String::from(COMPILED_AGENT_COMPATIBILITY_VERSION),
                confidence_floor: 0.8,
                artifact_id: route_model.artifact_id.clone(),
                artifact_digest: route_model.artifact_digest.clone(),
                row_id: default_row.row_id.clone(),
                default_row: default_row.clone(),
                evidence_class,
                validator_lineage: route_lineage.clone(),
                predecessor_artifact_id: Some(route_fallback_artifact_id.clone()),
                rollback_artifact_id: Some(route_fallback_artifact_id),
                promoted_at_utc: None,
                payload: CompiledAgentArtifactPayload::RouteModel {
                    artifact: route_model.clone(),
                },
                detail: String::from(
                    "Route candidate stays shadow-only under `psionic_candidate` because the widened held-out split produced a real ambiguity regression and the validator kept authority on the baseline route revision.",
                ),
            };
            (promoted_route, vec![route_candidate])
        }
    };

    let promoted_tool_policy = revision_entry(
        CompiledAgentModuleKind::ToolPolicy,
        "tool_policy",
        "tool_policy",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        CompiledAgentArtifactLifecycleState::Promoted,
        None,
        0.8,
        "compiled_agent.baseline.rule_v1.tool_policy".to_string(),
        &baseline_revision,
        &default_row,
        evidence_class,
        baseline_lineage.clone(),
        None,
        Some(String::from(PROMOTED_AT_UTC)),
        "Promoted tool-policy artifact remains the baseline bounded rule revision.",
    );

    let promoted_tool_arguments = revision_entry(
        CompiledAgentModuleKind::ToolArguments,
        "tool_arguments",
        "tool_arguments",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        CompiledAgentArtifactLifecycleState::Promoted,
        None,
        0.8,
        "compiled_agent.baseline.rule_v1.tool_arguments".to_string(),
        &baseline_revision,
        &default_row,
        evidence_class,
        baseline_lineage.clone(),
        None,
        Some(String::from(PROMOTED_AT_UTC)),
        "Promoted tool-argument artifact remains the baseline bounded rule revision.",
    );

    let promoted_grounded_answer = revision_entry(
        CompiledAgentModuleKind::GroundedAnswer,
        "grounded_answer",
        "grounded_answer",
        "psionic_grounded_model",
        "promoted",
        "2026-03-29",
        CompiledAgentArtifactLifecycleState::Promoted,
        None,
        0.82,
        grounded_candidate_revision.revision_id.clone(),
        &grounded_candidate_revision,
        &default_row,
        evidence_class,
        grounded_lineage.clone(),
        Some(String::from("compiled_agent.baseline.rule_v1.grounded_answer")),
        Some(String::from(PROMOTED_AT_UTC)),
        "Promoted grounded-answer artifact now embeds the learned fact-only grounded model retained in the compiled-agent XTRAIN loop.",
    );

    let rollback_grounded_answer = revision_entry(
        CompiledAgentModuleKind::GroundedAnswer,
        "grounded_answer",
        "grounded_answer",
        "psionic_rule_revision",
        "last_known_good",
        "2026-03-28",
        CompiledAgentArtifactLifecycleState::Candidate,
        Some("last_known_good"),
        0.82,
        "compiled_agent.baseline.rule_v1.grounded_answer".to_string(),
        &baseline_revision,
        &default_row,
        evidence_class,
        baseline_lineage.clone(),
        None,
        None,
        "Rollback grounded-answer artifact preserves the last-known-good baseline revision for clean rollback from the promoted learned grounded model.",
    );

    let promoted_verify = revision_entry(
        CompiledAgentModuleKind::Verify,
        "verify",
        "verify",
        "psionic_rule_revision",
        "promoted",
        "2026-03-28",
        CompiledAgentArtifactLifecycleState::Promoted,
        None,
        0.82,
        "compiled_agent.baseline.rule_v1.verify".to_string(),
        &baseline_revision,
        &default_row,
        evidence_class,
        baseline_lineage,
        None,
        Some(String::from(PROMOTED_AT_UTC)),
        "Promoted verify artifact remains the baseline rule revision.",
    );

    let candidate_verify = revision_entry(
        CompiledAgentModuleKind::Verify,
        "verify",
        "verify",
        "psionic_grounded_model",
        "psionic_candidate",
        "2026-03-29",
        CompiledAgentArtifactLifecycleState::Candidate,
        Some("psionic_candidate"),
        0.82,
        format!("{}.verify", grounded_candidate_revision.revision_id),
        &grounded_candidate_revision,
        &default_row,
        evidence_class,
        grounded_lineage,
        Some(String::from("compiled_agent.baseline.rule_v1.verify")),
        None,
        "Candidate verify artifact uses the same revision family as the current grounded-answer candidate so shadow runs preserve aligned grounded-answer verification semantics.",
    );

    let mut artifacts = vec![
        promoted_route,
        promoted_tool_policy,
        promoted_tool_arguments,
        promoted_grounded_answer,
        rollback_grounded_answer,
        promoted_verify,
        candidate_verify,
    ];
    artifacts.extend(route_candidate_entries);

    let promoted_entry_count = artifacts
        .iter()
        .filter(|entry| entry.lifecycle_state == CompiledAgentArtifactLifecycleState::Promoted)
        .count() as u32;
    let candidate_entry_count = artifacts.len() as u32 - promoted_entry_count;
    let mut entries_by_module = BTreeMap::new();
    for entry in &artifacts {
        *entries_by_module
            .entry(format!("{:?}", entry.module).to_ascii_lowercase())
            .or_insert(0) += 1;
    }

    let mut contract = CompiledAgentPromotedArtifactContract {
        schema_version: String::from(COMPILED_AGENT_PROMOTED_ARTIFACT_CONTRACT_SCHEMA_VERSION),
        ledger_id: String::from("compiled_agent.promoted_artifact_contract.v1"),
        row_id: default_row.row_id.clone(),
        evidence_class,
        promoted_entry_count,
        candidate_entry_count,
        entries_by_module,
        artifacts,
        summary: String::new(),
        contract_digest: String::new(),
    };
    contract.summary = format!(
        "Compiled-agent promoted-artifact contract retains {} promoted artifacts and {} candidate artifacts for the first graph under {:?} evidence. Route authority is currently `{}` after validator gating, and the promoted learned grounded-answer model still comes from {}.",
        contract.promoted_entry_count,
        contract.candidate_entry_count,
        contract.evidence_class,
        match xtrain_cycle.route_outcome.decision {
            CompiledAgentPromotionDecision::Promote => "compiled_agent.route.multinomial_nb_v1",
            CompiledAgentPromotionDecision::Hold => "compiled_agent.baseline.rule_v1.route",
        },
        GROUNDED_MODEL_FIXTURE_REF
    );
    contract.contract_digest =
        stable_digest(b"compiled_agent_promoted_artifact_contract|", &contract);
    Ok(contract)
}

pub fn write_compiled_agent_promoted_artifact_contract(
    output_path: impl AsRef<Path>,
) -> Result<CompiledAgentPromotedArtifactContract, CompiledAgentArtifactContractError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            CompiledAgentArtifactContractError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let contract = canonical_compiled_agent_promoted_artifact_contract()?;
    let json = serde_json::to_string_pretty(&contract)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        CompiledAgentArtifactContractError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(contract)
}

pub fn verify_compiled_agent_promoted_artifact_contract_fixture(
) -> Result<CompiledAgentPromotedArtifactContract, CompiledAgentArtifactContractError> {
    let path = compiled_agent_promoted_artifact_contract_fixture_path();
    let expected = canonical_compiled_agent_promoted_artifact_contract()?;
    let bytes = fs::read(&path).map_err(|error| CompiledAgentArtifactContractError::Read {
        path: path.display().to_string(),
        error,
    })?;
    let committed: CompiledAgentPromotedArtifactContract = serde_json::from_slice(&bytes)?;
    if committed.contract_digest != expected.contract_digest || committed != expected {
        return Err(CompiledAgentArtifactContractError::FixtureDrift {
            path: path.display().to_string(),
        });
    }
    Ok(committed)
}

fn read_grounded_model_artifact_fixture(
) -> Result<CompiledAgentGroundedAnswerModelArtifact, CompiledAgentArtifactContractError> {
    let path = repo_relative_path(GROUNDED_MODEL_FIXTURE_REF);
    let bytes = fs::read(&path).map_err(|error| CompiledAgentArtifactContractError::Read {
        path: path.display().to_string(),
        error,
    })?;
    Ok(serde_json::from_slice(&bytes)?)
}

fn grounded_candidate_revision_from_artifact(
    grounded_model_artifact: CompiledAgentGroundedAnswerModelArtifact,
) -> CompiledAgentModuleRevisionSet {
    let mut candidate = compiled_agent_baseline_revision_set();
    candidate.revision_id = grounded_model_artifact.artifact_id.clone();
    candidate.grounded_answer_model_artifact = Some(grounded_model_artifact);
    candidate.verify_require_recent_earnings = true;
    candidate
}

fn route_candidate_route_revision(
    route_model: CompiledAgentRouteModelArtifact,
) -> CompiledAgentModuleRevisionSet {
    let mut candidate = compiled_agent_baseline_revision_set();
    candidate.revision_id = route_model.artifact_id.clone();
    candidate.route_model_artifact = Some(route_model);
    candidate
}

fn revision_entry(
    module: CompiledAgentModuleKind,
    module_name: &str,
    signature_name: &str,
    implementation_family: &str,
    implementation_label: &str,
    version: &str,
    lifecycle_state: CompiledAgentArtifactLifecycleState,
    candidate_label: Option<&str>,
    confidence_floor: f32,
    artifact_id: String,
    revision: &CompiledAgentModuleRevisionSet,
    default_row: &CompiledAgentDefaultLearnedRowContract,
    evidence_class: CompiledAgentEvidenceClass,
    validator_lineage: CompiledAgentArtifactValidatorLineage,
    predecessor_artifact_id: Option<String>,
    promoted_at_utc: Option<String>,
    detail: &str,
) -> CompiledAgentArtifactContractEntry {
    CompiledAgentArtifactContractEntry {
        module,
        module_name: module_name.to_string(),
        signature_name: signature_name.to_string(),
        implementation_family: implementation_family.to_string(),
        implementation_label: implementation_label.to_string(),
        version: version.to_string(),
        lifecycle_state,
        candidate_label: candidate_label.map(ToOwned::to_owned),
        compatibility_version: String::from(COMPILED_AGENT_COMPATIBILITY_VERSION),
        confidence_floor,
        artifact_digest: revision_digest(revision),
        artifact_id,
        row_id: default_row.row_id.clone(),
        default_row: default_row.clone(),
        evidence_class,
        validator_lineage,
        predecessor_artifact_id,
        rollback_artifact_id: None,
        promoted_at_utc,
        payload: CompiledAgentArtifactPayload::RevisionSet {
            revision: revision.clone(),
        },
        detail: detail.to_string(),
    }
}

fn ensure_matching_evidence_class(
    expected: CompiledAgentEvidenceClass,
    actual: CompiledAgentEvidenceClass,
    context: &str,
) -> Result<(), CompiledAgentArtifactContractError> {
    if actual == expected {
        Ok(())
    } else {
        Err(CompiledAgentArtifactContractError::MixedEvidenceClass {
            context: context.to_string(),
            expected,
            actual,
        })
    }
}

fn validator_lineage(
    validator_report_ref: &str,
    validator_report: &CompiledAgentModuleEvalReport,
    xtrain_cycle_receipt_ref: Option<&str>,
    xtrain_cycle_receipt_digest: Option<&str>,
) -> CompiledAgentArtifactValidatorLineage {
    CompiledAgentArtifactValidatorLineage {
        validator_report_ref: validator_report_ref.to_string(),
        validator_report_digest: validator_report.report_digest.clone(),
        xtrain_cycle_receipt_ref: xtrain_cycle_receipt_ref.map(ToOwned::to_owned),
        xtrain_cycle_receipt_digest: xtrain_cycle_receipt_digest.map(ToOwned::to_owned),
    }
}

fn revision_digest(revision: &CompiledAgentModuleRevisionSet) -> String {
    stable_digest(b"compiled_agent_revision_artifact|", revision)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_compiled_agent_promoted_artifact_contract,
        canonical_compiled_agent_xtrain_cycle_receipt,
        verify_compiled_agent_promoted_artifact_contract_fixture,
        CompiledAgentArtifactLifecycleState, CompiledAgentPromotionDecision,
    };
    use psionic_eval::{CompiledAgentEvidenceClass, CompiledAgentModuleKind};

    #[test]
    fn promoted_artifact_contract_follows_route_validator_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_promoted_artifact_contract()?;
        let xtrain_cycle = canonical_compiled_agent_xtrain_cycle_receipt()?;
        assert_eq!(
            contract.evidence_class,
            CompiledAgentEvidenceClass::LearnedLane
        );
        let route = contract
            .promoted_entry(CompiledAgentModuleKind::Route)
            .expect("route entry missing");
        assert_eq!(
            route.lifecycle_state,
            CompiledAgentArtifactLifecycleState::Promoted
        );
        assert_eq!(
            route.evidence_class,
            CompiledAgentEvidenceClass::LearnedLane
        );
        match xtrain_cycle.route_outcome.decision {
            CompiledAgentPromotionDecision::Promote => {
                assert_eq!(route.artifact_id, "compiled_agent.route.multinomial_nb_v1");
                assert!(route.rollback_artifact_id.is_some());
            }
            CompiledAgentPromotionDecision::Hold => {
                assert_eq!(route.artifact_id, "compiled_agent.baseline.rule_v1.route");
                assert!(route.rollback_artifact_id.is_none());
                let candidate = contract
                    .candidate_entry(CompiledAgentModuleKind::Route, "psionic_candidate")
                    .expect("route candidate entry missing");
                assert_eq!(candidate.artifact_id, "compiled_agent.route.multinomial_nb_v1");
                assert_eq!(
                    candidate.lifecycle_state,
                    CompiledAgentArtifactLifecycleState::Candidate
                );
            }
        }
        Ok(())
    }

    #[test]
    fn promoted_artifact_contract_keeps_the_learned_grounded_answer_promoted(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = canonical_compiled_agent_promoted_artifact_contract()?;
        let grounded = contract
            .promoted_entry(CompiledAgentModuleKind::GroundedAnswer)
            .expect("grounded-answer entry missing");
        assert_eq!(
            grounded.lifecycle_state,
            CompiledAgentArtifactLifecycleState::Promoted
        );
        assert_eq!(
            grounded.artifact_id,
            "compiled_agent.grounded_answer.multinomial_nb_v1"
        );
        assert_eq!(grounded.implementation_family, "psionic_grounded_model");
        Ok(())
    }

    #[test]
    fn promoted_artifact_contract_fixture_matches_the_canonical_generator(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let contract = verify_compiled_agent_promoted_artifact_contract_fixture()?;
        assert!(contract.promoted_entry_count >= 5);
        Ok(())
    }
}
