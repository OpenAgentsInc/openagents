use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_post_article_canonical_route_semantic_preservation_audit_report,
    build_tassadar_post_article_canonical_route_universal_substrate_gate_report,
    build_tassadar_post_article_carrier_split_contract_report,
    build_tassadar_post_article_control_plane_decision_provenance_proof_report,
    build_tassadar_post_article_rebased_universality_verdict_split_report,
    build_tassadar_post_article_universal_machine_proof_rebinding_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_post_article_universality_portability_minimality_matrix_report,
    build_tassadar_post_article_universality_witness_suite_reissue_report,
    build_tassadar_turing_completeness_closeout_audit_report,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
    TassadarPostArticleCarrierSplitContractReport,
    TassadarPostArticleCarrierSplitContractReportError,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    TassadarPostArticleControlPlaneDecisionProvenanceProofReportError,
    TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    TassadarPostArticleRebasedUniversalityVerdictSplitReportError,
    TassadarPostArticleUniversalMachineProofRebindingReport,
    TassadarPostArticleUniversalMachineProofRebindingReportError,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
    TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    TassadarPostArticleUniversalityWitnessSuiteReissueReportError,
    TassadarTuringCompletenessCloseoutAuditReport,
    TassadarTuringCompletenessCloseoutAuditReportError, TassadarTuringCompletenessCloseoutStatus,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
    TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
    TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
    TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_turing_completeness_closeout_audit_report.json";
pub const TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-turing-completeness-closeout-audit.sh";

const TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_turing_completeness_closeout_summary.json";
const TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_plugin_capability_boundary_report.json";
const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_machine_closure_bundle_report.json";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleTuringCompletenessCloseoutStatus {
    TheoryGreenOperatorGreenServedSuppressed,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleTuringCompletenessSupportingMaterialClass {
    ProofCarrying,
    Summary,
    ObservationalContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTuringCompletenessSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleTuringCompletenessSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTuringCompletenessMachineIdentityBinding {
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub continuation_contract_digest: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub historical_closeout_report_id: String,
    pub historical_closeout_report_digest: String,
    pub rebased_verdict_report_id: String,
    pub rebased_verdict_report_digest: String,
    pub plugin_boundary_report_id: String,
    pub plugin_boundary_report_digest: String,
    pub closure_bundle_report_id: String,
    pub closure_bundle_report_digest: String,
    pub closure_bundle_digest: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTuringCompletenessDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTuringCompletenessValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleTuringCompletenessCloseoutAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub historical_closeout_audit_report_ref: String,
    pub historical_closeout_summary_ref: String,
    pub bridge_contract_report_ref: String,
    pub semantic_preservation_audit_report_ref: String,
    pub control_plane_decision_provenance_proof_report_ref: String,
    pub carrier_split_contract_report_ref: String,
    pub universal_machine_proof_rebinding_report_ref: String,
    pub universality_witness_suite_reissue_report_ref: String,
    pub canonical_route_universal_substrate_gate_report_ref: String,
    pub universality_portability_minimality_matrix_report_ref: String,
    pub rebased_universality_verdict_split_report_ref: String,
    pub plugin_capability_boundary_report_ref: String,
    pub canonical_machine_closure_bundle_report_ref: String,
    pub post_article_turing_audit_ref: String,
    pub plugin_system_turing_audit_ref: String,
    pub supporting_material_rows: Vec<TassadarPostArticleTuringCompletenessSupportingMaterialRow>,
    pub machine_identity_binding: TassadarPostArticleTuringCompletenessMachineIdentityBinding,
    pub dependency_rows: Vec<TassadarPostArticleTuringCompletenessDependencyRow>,
    pub validation_rows: Vec<TassadarPostArticleTuringCompletenessValidationRow>,
    pub closeout_status: TassadarPostArticleTuringCompletenessCloseoutStatus,
    pub closeout_green: bool,
    pub historical_tas_156_still_stands: bool,
    pub canonical_route_truth_carrier: bool,
    pub control_plane_proof_part_of_truth_carrier: bool,
    pub closure_bundle_embedded_here: bool,
    pub closure_bundle_bound_by_digest: bool,
    pub closure_bundle_issue_id: String,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub weighted_plugin_control_allowed: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleTuringCompletenessCloseoutAuditReportError {
    #[error(transparent)]
    HistoricalCloseout(#[from] TassadarTuringCompletenessCloseoutAuditReportError),
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
    #[error(transparent)]
    SemanticPreservation(
        #[from] TassadarPostArticleCanonicalRouteSemanticPreservationAuditReportError,
    ),
    #[error(transparent)]
    ControlPlane(#[from] TassadarPostArticleControlPlaneDecisionProvenanceProofReportError),
    #[error(transparent)]
    CarrierSplit(#[from] TassadarPostArticleCarrierSplitContractReportError),
    #[error(transparent)]
    ProofRebinding(#[from] TassadarPostArticleUniversalMachineProofRebindingReportError),
    #[error(transparent)]
    WitnessSuite(#[from] TassadarPostArticleUniversalityWitnessSuiteReissueReportError),
    #[error(transparent)]
    UniversalSubstrateGate(
        #[from] TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
    ),
    #[error(transparent)]
    PortabilityMatrix(
        #[from] TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
    ),
    #[error(transparent)]
    RebasedVerdict(#[from] TassadarPostArticleRebasedUniversalityVerdictSplitReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarTuringCompletenessCloseoutSummaryInput {
    report_id: String,
    allowed_statement: String,
    claim_boundary: String,
    report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarPostArticlePluginCapabilityBoundaryInput {
    report_id: String,
    machine_identity_binding: TassadarPostArticlePluginCapabilityMachineIdentityInput,
    boundary_green: bool,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    weighted_plugin_control_allowed: bool,
    plugin_publication_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
    report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarPostArticlePluginCapabilityMachineIdentityInput {
    machine_identity_id: String,
    canonical_model_id: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    continuation_contract_id: String,
    continuation_contract_digest: String,
    rebased_verdict_report_id: String,
    rebased_verdict_report_digest: String,
    canonical_architecture_anchor_crate: String,
    canonical_architecture_boundary_ref: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalMachineClosureBundleInput {
    report_id: String,
    report_digest: String,
    closure_bundle_digest: String,
    bundle_green: bool,
    closure_subject: CanonicalMachineClosureBundleSubjectInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalMachineClosureBundleSubjectInput {
    machine_identity_id: String,
    canonical_route_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SupportingDocumentSnapshot {
    source_ref: String,
    source_artifact_digest: String,
}

pub fn build_tassadar_post_article_turing_completeness_closeout_audit_report() -> Result<
    TassadarPostArticleTuringCompletenessCloseoutAuditReport,
    TassadarPostArticleTuringCompletenessCloseoutAuditReportError,
> {
    let historical_closeout = build_tassadar_turing_completeness_closeout_audit_report()?;
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let semantic_preservation =
        build_tassadar_post_article_canonical_route_semantic_preservation_audit_report()?;
    let control_plane =
        build_tassadar_post_article_control_plane_decision_provenance_proof_report()?;
    let carrier_split = build_tassadar_post_article_carrier_split_contract_report()?;
    let proof_rebinding = build_tassadar_post_article_universal_machine_proof_rebinding_report()?;
    let witness_suite = build_tassadar_post_article_universality_witness_suite_reissue_report()?;
    let universal_substrate_gate =
        build_tassadar_post_article_canonical_route_universal_substrate_gate_report()?;
    let portability_matrix =
        build_tassadar_post_article_universality_portability_minimality_matrix_report()?;
    let rebased_verdict = build_tassadar_post_article_rebased_universality_verdict_split_report()?;
    let historical_summary: TassadarTuringCompletenessCloseoutSummaryInput =
        read_repo_json(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF_LOCAL)?;
    let plugin_boundary: TassadarPostArticlePluginCapabilityBoundaryInput =
        read_repo_json(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF_LOCAL)?;
    let closure_bundle: CanonicalMachineClosureBundleInput = read_repo_json(
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
    )?;
    let post_article_turing_audit = read_supporting_document(POST_ARTICLE_TURING_AUDIT_REF)?;
    let plugin_system_turing_audit = read_supporting_document(PLUGIN_SYSTEM_TURING_AUDIT_REF)?;

    Ok(build_report_from_inputs(
        closure_bundle,
        historical_closeout,
        historical_summary,
        bridge,
        semantic_preservation,
        control_plane,
        carrier_split,
        proof_rebinding,
        witness_suite,
        universal_substrate_gate,
        portability_matrix,
        rebased_verdict,
        plugin_boundary,
        post_article_turing_audit,
        plugin_system_turing_audit,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    closure_bundle: CanonicalMachineClosureBundleInput,
    historical_closeout: TassadarTuringCompletenessCloseoutAuditReport,
    historical_summary: TassadarTuringCompletenessCloseoutSummaryInput,
    bridge: TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: TassadarPostArticleCarrierSplitContractReport,
    proof_rebinding: TassadarPostArticleUniversalMachineProofRebindingReport,
    witness_suite: TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    universal_substrate_gate: TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    portability_matrix: TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    rebased_verdict: TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    plugin_boundary: TassadarPostArticlePluginCapabilityBoundaryInput,
    post_article_turing_audit: SupportingDocumentSnapshot,
    plugin_system_turing_audit: SupportingDocumentSnapshot,
) -> TassadarPostArticleTuringCompletenessCloseoutAuditReport {
    let historical_tas_156_still_stands = historical_closeout.claim_status
        == TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed;
    let canonical_route_truth_carrier = bridge.bridge_contract_green
        && semantic_preservation.semantic_preservation_audit_green
        && control_plane.decision_provenance_proof_complete
        && carrier_split.carrier_split_publication_complete
        && proof_rebinding.proof_rebinding_complete
        && witness_suite.witness_suite_reissued
        && universal_substrate_gate.gate_green
        && universal_substrate_gate.universal_substrate_gate_allowed
        && portability_matrix.matrix_green
        && portability_matrix.bounded_universality_story_carried
        && rebased_verdict.verdict_split_green
        && rebased_verdict.rebase_claim_allowed;
    let control_plane_proof_part_of_truth_carrier = canonical_route_truth_carrier
        && control_plane.control_plane_ownership_green
        && control_plane.decision_provenance_proof_complete;
    let closure_bundle_embedded_here = false;
    let closure_bundle_bound_by_digest = closure_bundle.bundle_green
        && closure_bundle.closure_subject.machine_identity_id
            == bridge.bridge_machine_identity.machine_identity_id
        && closure_bundle.closure_subject.canonical_route_id
            == bridge.bridge_machine_identity.canonical_route_id;
    let theory_green = rebased_verdict.theory_green;
    let operator_green = rebased_verdict.operator_green;
    let served_green = rebased_verdict.served_green;
    let rebase_claim_allowed =
        rebased_verdict.rebase_claim_allowed && plugin_boundary.rebase_claim_allowed;
    let plugin_capability_claim_allowed = rebased_verdict.plugin_capability_claim_allowed
        || plugin_boundary.plugin_capability_claim_allowed;
    let weighted_plugin_control_allowed = plugin_boundary.weighted_plugin_control_allowed;
    let plugin_publication_allowed = plugin_boundary.plugin_publication_allowed;
    let served_public_universality_allowed = rebased_verdict.served_public_universality_allowed
        || plugin_boundary.served_public_universality_allowed;
    let arbitrary_software_capability_allowed = rebased_verdict
        .arbitrary_software_capability_allowed
        || plugin_boundary.arbitrary_software_capability_allowed;

    let machine_identity_binding = TassadarPostArticleTuringCompletenessMachineIdentityBinding {
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
        canonical_weight_artifact_id: bridge
            .bridge_machine_identity
            .canonical_weight_artifact_id
            .clone(),
        canonical_weight_bundle_digest: bridge
            .bridge_machine_identity
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: bridge
            .bridge_machine_identity
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        canonical_route_descriptor_digest: bridge
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .clone(),
        continuation_contract_id: bridge.bridge_machine_identity.continuation_contract_id.clone(),
        continuation_contract_digest: bridge
            .bridge_machine_identity
            .continuation_contract_digest
            .clone(),
        canonical_architecture_anchor_crate: plugin_boundary
            .machine_identity_binding
            .canonical_architecture_anchor_crate
            .clone(),
        canonical_architecture_boundary_ref: plugin_boundary
            .machine_identity_binding
            .canonical_architecture_boundary_ref
            .clone(),
        historical_closeout_report_id: historical_closeout.report_id.clone(),
        historical_closeout_report_digest: historical_closeout.report_digest.clone(),
        rebased_verdict_report_id: plugin_boundary
            .machine_identity_binding
            .rebased_verdict_report_id
            .clone(),
        rebased_verdict_report_digest: plugin_boundary
            .machine_identity_binding
            .rebased_verdict_report_digest
            .clone(),
        plugin_boundary_report_id: plugin_boundary.report_id.clone(),
        plugin_boundary_report_digest: plugin_boundary.report_digest.clone(),
        closure_bundle_report_id: closure_bundle.report_id.clone(),
        closure_bundle_report_digest: closure_bundle.report_digest.clone(),
        closure_bundle_digest: closure_bundle.closure_bundle_digest.clone(),
        detail: format!(
            "machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` keeps the historical closeout bound to the post-`TAS-186` canonical route while carrying the control-plane proof, rebased verdict, plugin boundary, and closure_bundle_digest=`{}` on the same machine identity without embedding the closure bundle here.",
            bridge.bridge_machine_identity.machine_identity_id,
            bridge.bridge_machine_identity.canonical_model_id,
            bridge.bridge_machine_identity.canonical_route_id,
            closure_bundle.closure_bundle_digest,
        ),
    };

    let supporting_material_rows = build_supporting_material_rows(
        &historical_closeout,
        &historical_summary,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
        &proof_rebinding,
        &witness_suite,
        &universal_substrate_gate,
        &portability_matrix,
        &rebased_verdict,
        &plugin_boundary,
        &post_article_turing_audit,
        &plugin_system_turing_audit,
    );
    let dependency_rows = build_dependency_rows(
        historical_tas_156_still_stands,
        &bridge,
        &semantic_preservation,
        &control_plane,
        &carrier_split,
        &proof_rebinding,
        &witness_suite,
        &universal_substrate_gate,
        &portability_matrix,
        &rebased_verdict,
        &plugin_boundary,
        &closure_bundle,
    );
    let validation_rows = build_validation_rows(
        &supporting_material_rows,
        &bridge,
        &control_plane,
        &carrier_split,
        &proof_rebinding,
        &universal_substrate_gate,
        &portability_matrix,
        &rebased_verdict,
        &plugin_boundary,
        closure_bundle_bound_by_digest,
        closure_bundle_embedded_here,
    );

    let closeout_green = dependency_rows.iter().all(|row| row.satisfied)
        && validation_rows.iter().all(|row| row.green)
        && historical_tas_156_still_stands
        && canonical_route_truth_carrier
        && control_plane_proof_part_of_truth_carrier
        && closure_bundle_bound_by_digest
        && theory_green
        && operator_green
        && !served_green
        && rebase_claim_allowed
        && !plugin_capability_claim_allowed
        && !weighted_plugin_control_allowed
        && !plugin_publication_allowed
        && !served_public_universality_allowed
        && !arbitrary_software_capability_allowed;
    let closeout_status = if closeout_green {
        TassadarPostArticleTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed
    } else {
        TassadarPostArticleTuringCompletenessCloseoutStatus::Incomplete
    };

    let mut report = TassadarPostArticleTuringCompletenessCloseoutAuditReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_turing_completeness_closeout_audit.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_CHECKER_REF,
        ),
        historical_closeout_audit_report_ref: String::from(
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
        ),
        historical_closeout_summary_ref: String::from(
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF_LOCAL,
        ),
        bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
        ),
        semantic_preservation_audit_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
        ),
        control_plane_decision_provenance_proof_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
        ),
        carrier_split_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
        ),
        universal_machine_proof_rebinding_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
        ),
        universality_witness_suite_reissue_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
        ),
        canonical_route_universal_substrate_gate_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
        ),
        universality_portability_minimality_matrix_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
        ),
        rebased_universality_verdict_split_report_ref: String::from(
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        plugin_capability_boundary_report_ref: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF_LOCAL,
        ),
        canonical_machine_closure_bundle_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
        ),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        plugin_system_turing_audit_ref: String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
        supporting_material_rows,
        machine_identity_binding,
        dependency_rows,
        validation_rows,
        closeout_status,
        closeout_green,
        historical_tas_156_still_stands,
        canonical_route_truth_carrier,
        control_plane_proof_part_of_truth_carrier,
        closure_bundle_embedded_here,
        closure_bundle_bound_by_digest,
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        theory_green,
        operator_green,
        served_green,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        weighted_plugin_control_allowed,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        claim_boundary: format!(
            "this audit keeps the historical `TAS-156` closeout standing, reissues the bounded Turing-completeness claim on the canonical post-`TAS-186` route, states machine-readably that control-plane ownership plus decision provenance are part of that truth carrier, and now inherits the canonical machine closure bundle by report id and digest. It is not the canonical machine closure bundle itself, which remains a separate artifact published under `{}`.",
            CLOSURE_BUNDLE_ISSUE_ID,
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article turing-completeness closeout audit keeps supporting_materials={}/{}, dependency_rows={}/{}, validation_rows={}/{}, closeout_status={:?}, closure_bundle_digest=`{}`, and closure_bundle_issue_id=`{}`.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.supporting_material_rows.len(),
        report.dependency_rows.iter().filter(|row| row.satisfied).count(),
        report.dependency_rows.len(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.closeout_status,
        report.machine_identity_binding.closure_bundle_digest,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_turing_completeness_closeout_audit_report|",
        &report,
    );
    report
}

fn build_supporting_material_rows(
    historical_closeout: &TassadarTuringCompletenessCloseoutAuditReport,
    historical_summary: &TassadarTuringCompletenessCloseoutSummaryInput,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    witness_suite: &TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    portability_matrix: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    rebased_verdict: &TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    plugin_boundary: &TassadarPostArticlePluginCapabilityBoundaryInput,
    post_article_turing_audit: &SupportingDocumentSnapshot,
    plugin_system_turing_audit: &SupportingDocumentSnapshot,
) -> Vec<TassadarPostArticleTuringCompletenessSupportingMaterialRow> {
    vec![
        supporting_material_row(
            "historical_tas_156_closeout_audit",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            historical_closeout.claim_status
                == TassadarTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed,
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
            Some(historical_closeout.report_id.clone()),
            Some(historical_closeout.report_digest.clone()),
            "the historical bounded Turing-completeness closeout must stay green and preserved without rewrite before the canonical-route rebase can be published",
        ),
        supporting_material_row(
            "historical_tas_156_closeout_summary",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::Summary,
            !historical_summary.allowed_statement.is_empty()
                && !historical_summary.claim_boundary.is_empty(),
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF_LOCAL,
            Some(historical_summary.report_id.clone()),
            Some(historical_summary.report_digest.clone()),
            "the older disclosure-safe closeout summary remains cited as historical context rather than being silently superseded",
        ),
        supporting_material_row(
            "post_article_universality_bridge_contract",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the canonical post-`TAS-186` machine identity tuple and carrier split must stay bound to the historical substrate before the rebased closeout can be published",
        ),
        supporting_material_row(
            "post_article_canonical_route_semantic_preservation_audit",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            semantic_preservation.semantic_preservation_audit_green,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            Some(semantic_preservation.report_id.clone()),
            Some(semantic_preservation.report_digest.clone()),
            "the canonical route must preserve declared semantics and state ownership on the bridge machine identity before the rebased closeout can carry the claim",
        ),
        supporting_material_row(
            "post_article_control_plane_decision_provenance_proof",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            control_plane.decision_provenance_proof_complete
                && control_plane.control_plane_ownership_green,
            TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            Some(control_plane.report_id.clone()),
            Some(control_plane.report_digest.clone()),
            "control-plane ownership and decision provenance are part of the canonical-route truth carrier rather than a side note outside the claim",
        ),
        supporting_material_row(
            "post_article_carrier_split_contract",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            carrier_split.carrier_split_publication_complete,
            TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF,
            Some(carrier_split.report_id.clone()),
            Some(carrier_split.report_digest.clone()),
            "the direct article-equivalent lane and resumable bounded-universality lane must stay explicitly split instead of collapsing into one implied carrier",
        ),
        supporting_material_row(
            "post_article_universal_machine_proof_rebinding",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            proof_rebinding.proof_rebinding_complete,
            TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
            Some(proof_rebinding.report_id.clone()),
            Some(proof_rebinding.report_digest.clone()),
            "the historical universal-machine proof must remain rebound to the canonical machine identity through one explicit proof-transport boundary",
        ),
        supporting_material_row(
            "post_article_universality_witness_suite_reissue",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            witness_suite.witness_suite_reissued,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
            Some(witness_suite.report_id.clone()),
            Some(witness_suite.report_digest.clone()),
            "the witness suite must stay reissued on the canonical route instead of being inherited from the historical closeout by implication",
        ),
        supporting_material_row(
            "post_article_canonical_route_universal_substrate_gate",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            universal_substrate_gate.gate_green
                && universal_substrate_gate.universal_substrate_gate_allowed,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            Some(universal_substrate_gate.report_id.clone()),
            Some(universal_substrate_gate.report_digest.clone()),
            "the canonical route must still satisfy the rebased universal-substrate gate before the closeout can speak for the owned route",
        ),
        supporting_material_row(
            "post_article_universality_portability_minimality_matrix",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            portability_matrix.matrix_green && portability_matrix.bounded_universality_story_carried,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
            Some(portability_matrix.report_id.clone()),
            Some(portability_matrix.report_digest.clone()),
            "the declared machine matrix, route classification, minimality contract, and served suppression envelope must remain explicit and green",
        ),
        supporting_material_row(
            "post_article_rebased_universality_verdict_split",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            rebased_verdict.verdict_split_green && rebased_verdict.rebase_claim_allowed,
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            Some(rebased_verdict.report_id.clone()),
            Some(rebased_verdict.report_digest.clone()),
            "the final theory/operator verdict split on the canonical machine identity must stay explicit before the rebased closeout can be published",
        ),
        supporting_material_row(
            "post_article_plugin_capability_boundary",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ProofCarrying,
            plugin_boundary.boundary_green
                && !plugin_boundary.plugin_capability_claim_allowed
                && !plugin_boundary.plugin_publication_allowed,
            TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF_LOCAL,
            Some(plugin_boundary.report_id.clone()),
            Some(plugin_boundary.report_digest.clone()),
            "the plugin-aware boundary must stay explicit so the rebased closeout does not silently widen into weighted plugin control or plugin publication",
        ),
        supporting_material_row(
            "post_article_turing_completeness_audit_doc",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ObservationalContext,
            true,
            &post_article_turing_audit.source_ref,
            None,
            Some(post_article_turing_audit.source_artifact_digest.clone()),
            "the operator-readable audit remains cited as observational context and not as a proof-carrying substitute for the machine-readable artifacts",
        ),
        supporting_material_row(
            "plugin_system_and_turing_audit_doc",
            TassadarPostArticleTuringCompletenessSupportingMaterialClass::ObservationalContext,
            true,
            &plugin_system_turing_audit.source_ref,
            None,
            Some(plugin_system_turing_audit.source_artifact_digest.clone()),
            "the plugin-system audit remains cited as observational context for scope and publication boundaries rather than as the proof carrier for the rebased closeout",
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_dependency_rows(
    historical_tas_156_still_stands: bool,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    semantic_preservation: &TassadarPostArticleCanonicalRouteSemanticPreservationAuditReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    witness_suite: &TassadarPostArticleUniversalityWitnessSuiteReissueReport,
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    portability_matrix: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    rebased_verdict: &TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    plugin_boundary: &TassadarPostArticlePluginCapabilityBoundaryInput,
    closure_bundle: &CanonicalMachineClosureBundleInput,
) -> Vec<TassadarPostArticleTuringCompletenessDependencyRow> {
    let historical_binding_preserved = bridge.historical_binding_rows.iter().any(|row| {
        row.historical_artifact_ref == TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF
            && row.preserved_without_rewrite
            && row.canonical_machine_identity_bound
    });
    vec![
        dependency_row(
            "historical_tas_156_closeout_preserved",
            historical_tas_156_still_stands && historical_binding_preserved,
            vec![
                String::from(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            "the historical `TAS-156` closeout must still stand and be preserved without rewrite on the canonical bridge machine identity",
        ),
        dependency_row(
            "canonical_bridge_contract_green",
            bridge.bridge_contract_green,
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "the canonical post-`TAS-186` bridge contract is the starting identity carrier for the rebased closeout",
        ),
        dependency_row(
            "canonical_route_semantic_preservation_green",
            semantic_preservation.semantic_preservation_audit_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_SEMANTIC_PRESERVATION_AUDIT_REPORT_REF,
            )],
            "the canonical route must preserve semantics and declared state ownership before it can carry the rebased closeout",
        ),
        dependency_row(
            "control_plane_decision_provenance_complete",
            control_plane.control_plane_ownership_green
                && control_plane.decision_provenance_proof_complete,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
            )],
            "control-plane ownership and decision provenance are part of the truth carrier for the canonical-route claim",
        ),
        dependency_row(
            "carrier_split_publication_complete",
            carrier_split.carrier_split_publication_complete,
            vec![String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF)],
            "the direct article-equivalent lane and resumable bounded-universality lane must remain split explicitly",
        ),
        dependency_row(
            "universal_machine_proof_rebinding_complete",
            proof_rebinding.proof_rebinding_complete,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF,
            )],
            "the historical universal-machine proof must remain rebound to the canonical machine identity",
        ),
        dependency_row(
            "universality_witness_suite_reissued",
            witness_suite.witness_suite_reissued,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_WITNESS_SUITE_REISSUE_REPORT_REF,
            )],
            "the witness suite must stay reissued on the canonical route rather than inherited by implication",
        ),
        dependency_row(
            "canonical_route_universal_substrate_gate_green",
            universal_substrate_gate.gate_green
                && universal_substrate_gate.universal_substrate_gate_allowed,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "the canonical route must still satisfy the rebased universal-substrate gate",
        ),
        dependency_row(
            "universality_portability_minimality_matrix_green",
            portability_matrix.matrix_green
                && portability_matrix.machine_matrix_green
                && portability_matrix.bounded_universality_story_carried,
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
            )],
            "the machine matrix, route classification, minimality contract, and served suppression envelope must remain green",
        ),
        dependency_row(
            "rebased_verdict_split_green",
            rebased_verdict.verdict_split_green
                && rebased_verdict.rebase_claim_allowed
                && rebased_verdict.theory_green
                && rebased_verdict.operator_green
                && !rebased_verdict.served_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            )],
            "the rebased theory/operator verdict split must remain green while served/public universality stays suppressed",
        ),
        dependency_row(
            "plugin_boundary_scope_preserved",
            plugin_boundary.boundary_green
                && !plugin_boundary.plugin_capability_claim_allowed
                && !plugin_boundary.weighted_plugin_control_allowed
                && !plugin_boundary.plugin_publication_allowed,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF_LOCAL,
            )],
            "the closeout remains plugin-aware only because the plugin boundary keeps weighted/plugin/public widening explicitly out of scope",
        ),
        dependency_row(
            "canonical_machine_closure_bundle_published",
            closure_bundle.bundle_green,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
            )],
            "the terminal bounded Turing-completeness claim must reference the canonical machine closure bundle by digest instead of recomposing the machine from adjacent proof and boundary artifacts.",
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    supporting_material_rows: &[TassadarPostArticleTuringCompletenessSupportingMaterialRow],
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    control_plane: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    carrier_split: &TassadarPostArticleCarrierSplitContractReport,
    proof_rebinding: &TassadarPostArticleUniversalMachineProofRebindingReport,
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    portability_matrix: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    rebased_verdict: &TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    plugin_boundary: &TassadarPostArticlePluginCapabilityBoundaryInput,
    closure_bundle_bound_by_digest: bool,
    closure_bundle_embedded_here: bool,
) -> Vec<TassadarPostArticleTuringCompletenessValidationRow> {
    let helper_substitution_green =
        control_plane_has_validation_id(control_plane, "helper_substitution_quarantined")
            && carrier_split_has_validation_id(carrier_split, "helper_substitution_quarantined")
            && proof_rebinding_has_validation_id(
                proof_rebinding,
                "helper_substitution_quarantined",
            )
            && universal_substrate_gate_has_validation_id(
                universal_substrate_gate,
                "helper_substitution_quarantined",
            )
            && portability_matrix_has_validation_id(
                portability_matrix,
                "helper_substitution_quarantined",
            )
            && rebased_verdict_has_validation_id(
                rebased_verdict,
                "helper_substitution_quarantined",
            );
    let route_drift_green = control_plane_has_validation_id(control_plane, "route_drift_rejected")
        && carrier_split_has_validation_id(carrier_split, "route_drift_rejected")
        && proof_rebinding_has_validation_id(proof_rebinding, "route_drift_rejected")
        && universal_substrate_gate_has_validation_id(
            universal_substrate_gate,
            "route_drift_rejected",
        )
        && portability_matrix_has_validation_id(portability_matrix, "route_drift_rejected")
        && rebased_verdict_has_validation_id(rebased_verdict, "route_drift_rejected");
    let continuation_abuse_green =
        control_plane_has_validation_id(control_plane, "continuation_abuse_quarantined")
            && carrier_split_has_validation_id(carrier_split, "continuation_abuse_quarantined")
            && proof_rebinding_has_validation_id(proof_rebinding, "continuation_abuse_quarantined")
            && universal_substrate_gate_has_validation_id(
                universal_substrate_gate,
                "continuation_abuse_quarantined",
            )
            && portability_matrix_has_validation_id(
                portability_matrix,
                "continuation_abuse_quarantined",
            )
            && rebased_verdict_has_validation_id(rebased_verdict, "continuation_abuse_quarantined");
    let semantic_drift_green =
        control_plane_has_validation_id(control_plane, "semantic_drift_blocked")
            && carrier_split_has_validation_id(carrier_split, "semantic_drift_blocked")
            && proof_rebinding_has_validation_id(proof_rebinding, "semantic_drift_blocked")
            && universal_substrate_gate_has_validation_id(
                universal_substrate_gate,
                "semantic_drift_blocked",
            )
            && portability_matrix_has_validation_id(portability_matrix, "semantic_drift_blocked")
            && rebased_verdict_has_validation_id(rebased_verdict, "semantic_drift_blocked");
    let overclaim_green =
        control_plane_has_validation_id(control_plane, "overclaim_posture_explicit")
            && carrier_split_has_validation_id(carrier_split, "overclaim_posture_explicit")
            && proof_rebinding_has_validation_id(proof_rebinding, "overclaim_posture_explicit")
            && universal_substrate_gate_has_validation_id(
                universal_substrate_gate,
                "overclaim_posture_explicit",
            )
            && portability_matrix_has_validation_id(
                portability_matrix,
                "overclaim_posture_explicit",
            )
            && rebased_verdict_has_validation_id(rebased_verdict, "overclaim_posture_explicit");
    let proof_vs_audit_distinction_explicit = control_plane_has_validation_id(
        control_plane,
        "proof_class_distinction_preserved",
    ) && carrier_split_has_validation_id(carrier_split, "proof_class_distinction_preserved")
        && proof_rebinding_has_validation_id(
            proof_rebinding,
            "proof_carrying_distinction_preserved",
        )
        && supporting_material_rows.iter().any(|row| {
            row.material_id == "post_article_turing_completeness_audit_doc"
                && row.material_class
                    == TassadarPostArticleTuringCompletenessSupportingMaterialClass::ObservationalContext
        })
        && supporting_material_rows.iter().any(|row| {
            row.material_id == "plugin_system_and_turing_audit_doc"
                && row.material_class
                    == TassadarPostArticleTuringCompletenessSupportingMaterialClass::ObservationalContext
        });
    let historical_closeout_preserved_without_rewrite =
        bridge.historical_binding_rows.iter().any(|row| {
            row.historical_artifact_ref == TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF
                && row.preserved_without_rewrite
                && row.canonical_machine_identity_bound
        }) && rebased_verdict_has_validation_id(
            rebased_verdict,
            "historical_artifacts_preserved_without_rewrite",
        );
    let served_public_boundary_preserved = portability_matrix_has_validation_id(
        portability_matrix,
        "served_suppression_boundary_preserved",
    ) && !rebased_verdict.served_green
        && !rebased_verdict.served_public_universality_allowed
        && !plugin_boundary.served_public_universality_allowed;
    let plugin_scope_remains_out_of_scope = plugin_boundary.boundary_green
        && !plugin_boundary.plugin_capability_claim_allowed
        && !plugin_boundary.weighted_plugin_control_allowed
        && !plugin_boundary.plugin_publication_allowed
        && !plugin_boundary.arbitrary_software_capability_allowed;

    vec![
        validation_row(
            "helper_substitution_quarantined",
            helper_substitution_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            ],
            "helper substitution remains quarantined across the proof-carrying rebased closeout chain",
        ),
        validation_row(
            "route_drift_rejected",
            route_drift_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            ],
            "route drift remains rejected across the proof-carrying rebased closeout chain",
        ),
        validation_row(
            "continuation_abuse_quarantined",
            continuation_abuse_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            ],
            "continuation abuse remains quarantined across the proof-carrying rebased closeout chain",
        ),
        validation_row(
            "semantic_drift_blocked",
            semantic_drift_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
            ],
            "semantic drift remains blocked across the proof-carrying rebased closeout chain",
        ),
        validation_row(
            "overclaim_posture_explicit",
            overclaim_green,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF_LOCAL),
            ],
            "overclaim posture remains explicit across the rebased closeout, served boundary, and plugin boundary surfaces",
        ),
        validation_row(
            "proof_vs_audit_distinction_explicit",
            proof_vs_audit_distinction_explicit,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CONTROL_PLANE_DECISION_PROVENANCE_PROOF_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_CARRIER_SPLIT_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSAL_MACHINE_PROOF_REBINDING_REPORT_REF),
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
            ],
            "proof-carrying artifacts stay distinct from observational audit documents in the final rebased closeout",
        ),
        validation_row(
            "closure_bundle_remains_separate",
            !closure_bundle_embedded_here,
            vec![String::from(POST_ARTICLE_TURING_AUDIT_REF)],
            "the final claim-bearing canonical machine closure bundle remains separate from this audit even though the audit now inherits it by digest",
        ),
        validation_row(
            "closure_bundle_bound_by_digest",
            closure_bundle_bound_by_digest,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CLOSURE_BUNDLE_REPORT_REF_LOCAL,
            )],
            "the bounded Turing-completeness claim now inherits the canonical machine closure bundle by digest instead of relying on adjacent machine fields only",
        ),
        validation_row(
            "historical_closeout_preserved_without_rewrite",
            historical_closeout_preserved_without_rewrite,
            vec![
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                String::from(TASSADAR_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF),
            ],
            "the historical closeout remains preserved without rewrite while the canonical-route bridge rebinds it explicitly",
        ),
        validation_row(
            "served_public_boundary_preserved",
            served_public_boundary_preserved,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF_LOCAL),
            ],
            "served/public universality remains suppressed even though the canonical-route theory/operator closeout is now green",
        ),
        validation_row(
            "plugin_scope_remains_out_of_scope",
            plugin_scope_remains_out_of_scope,
            vec![String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_CAPABILITY_BOUNDARY_REPORT_REF_LOCAL,
            )],
            "plugin capability, weighted plugin control, plugin publication, and arbitrary software capability remain out of scope for this closeout",
        ),
    ]
}

#[must_use]
pub fn tassadar_post_article_turing_completeness_closeout_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF)
}

pub fn write_tassadar_post_article_turing_completeness_closeout_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleTuringCompletenessCloseoutAuditReport,
    TassadarPostArticleTuringCompletenessCloseoutAuditReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleTuringCompletenessCloseoutAuditReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_turing_completeness_closeout_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleTuringCompletenessCloseoutAuditReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn read_supporting_document(
    relative_path: &str,
) -> Result<SupportingDocumentSnapshot, TassadarPostArticleTuringCompletenessCloseoutAuditReportError>
{
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleTuringCompletenessCloseoutAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    Ok(SupportingDocumentSnapshot {
        source_ref: String::from(relative_path),
        source_artifact_digest: stable_digest_bytes(
            b"psionic_tassadar_post_article_turing_completeness_supporting_doc|",
            &bytes,
        ),
    })
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleTuringCompletenessCloseoutAuditReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleTuringCompletenessCloseoutAuditReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleTuringCompletenessCloseoutAuditReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

fn supporting_material_row(
    material_id: impl Into<String>,
    material_class: TassadarPostArticleTuringCompletenessSupportingMaterialClass,
    satisfied: bool,
    source_ref: impl Into<String>,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: impl Into<String>,
) -> TassadarPostArticleTuringCompletenessSupportingMaterialRow {
    TassadarPostArticleTuringCompletenessSupportingMaterialRow {
        material_id: material_id.into(),
        material_class,
        satisfied,
        source_ref: source_ref.into(),
        source_artifact_id,
        source_artifact_digest,
        detail: detail.into(),
    }
}

fn dependency_row(
    dependency_id: impl Into<String>,
    satisfied: bool,
    source_refs: Vec<String>,
    detail: impl Into<String>,
) -> TassadarPostArticleTuringCompletenessDependencyRow {
    TassadarPostArticleTuringCompletenessDependencyRow {
        dependency_id: dependency_id.into(),
        satisfied,
        source_refs,
        detail: detail.into(),
    }
}

fn validation_row(
    validation_id: impl Into<String>,
    green: bool,
    source_refs: Vec<String>,
    detail: impl Into<String>,
) -> TassadarPostArticleTuringCompletenessValidationRow {
    TassadarPostArticleTuringCompletenessValidationRow {
        validation_id: validation_id.into(),
        green,
        source_refs,
        detail: detail.into(),
    }
}

fn control_plane_has_validation_id(
    report: &TassadarPostArticleControlPlaneDecisionProvenanceProofReport,
    validation_id: &str,
) -> bool {
    report
        .validation_rows
        .iter()
        .any(|row| row.validation_id == validation_id && row.green)
}

fn carrier_split_has_validation_id(
    report: &TassadarPostArticleCarrierSplitContractReport,
    validation_id: &str,
) -> bool {
    report
        .validation_rows
        .iter()
        .any(|row| row.validation_id == validation_id && row.green)
}

fn proof_rebinding_has_validation_id(
    report: &TassadarPostArticleUniversalMachineProofRebindingReport,
    validation_id: &str,
) -> bool {
    report
        .validation_rows
        .iter()
        .any(|row| row.validation_id == validation_id && row.green)
}

fn universal_substrate_gate_has_validation_id(
    report: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    validation_id: &str,
) -> bool {
    report
        .validation_rows
        .iter()
        .any(|row| row.validation_id == validation_id && row.green)
}

fn portability_matrix_has_validation_id(
    report: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    validation_id: &str,
) -> bool {
    report
        .validation_rows
        .iter()
        .any(|row| row.validation_id == validation_id && row.green)
}

fn rebased_verdict_has_validation_id(
    report: &TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    validation_id: &str,
) -> bool {
    report
        .validation_rows
        .iter()
        .any(|row| row.validation_id == validation_id && row.green)
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn stable_digest_bytes(prefix: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_committed_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleTuringCompletenessCloseoutAuditReportError> {
    read_repo_json(relative_path)
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_turing_completeness_closeout_audit_report, read_committed_json,
        tassadar_post_article_turing_completeness_closeout_audit_report_path,
        write_tassadar_post_article_turing_completeness_closeout_audit_report,
        TassadarPostArticleTuringCompletenessCloseoutAuditReport,
        TassadarPostArticleTuringCompletenessCloseoutStatus,
        TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn post_article_turing_completeness_closeout_audit_turns_green_when_prereqs_hold() {
        let report = build_tassadar_post_article_turing_completeness_closeout_audit_report()
            .expect("report");

        assert_eq!(
            report.closeout_status,
            TassadarPostArticleTuringCompletenessCloseoutStatus::TheoryGreenOperatorGreenServedSuppressed
        );
        assert!(report.closeout_green);
        assert_eq!(report.supporting_material_rows.len(), 14);
        assert_eq!(report.dependency_rows.len(), 12);
        assert_eq!(report.validation_rows.len(), 11);
        assert!(report.historical_tas_156_still_stands);
        assert!(report.canonical_route_truth_carrier);
        assert!(report.control_plane_proof_part_of_truth_carrier);
        assert!(report.closure_bundle_bound_by_digest);
        assert!(!report.closure_bundle_embedded_here);
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
        assert!(report.theory_green);
        assert!(report.operator_green);
        assert!(!report.served_green);
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.weighted_plugin_control_allowed);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
    }

    #[test]
    fn post_article_turing_completeness_closeout_audit_matches_committed_truth() {
        let generated = build_tassadar_post_article_turing_completeness_closeout_audit_report()
            .expect("report");
        let committed: TassadarPostArticleTuringCompletenessCloseoutAuditReport =
            read_committed_json(
                TASSADAR_POST_ARTICLE_TURING_COMPLETENESS_CLOSEOUT_AUDIT_REPORT_REF,
            )
            .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_turing_completeness_closeout_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_turing_completeness_closeout_audit_report.json")
        );
    }

    #[test]
    fn write_post_article_turing_completeness_closeout_audit_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_turing_completeness_closeout_audit_report.json");
        let written =
            write_tassadar_post_article_turing_completeness_closeout_audit_report(&output_path)
                .expect("write report");
        let persisted: TassadarPostArticleTuringCompletenessCloseoutAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
