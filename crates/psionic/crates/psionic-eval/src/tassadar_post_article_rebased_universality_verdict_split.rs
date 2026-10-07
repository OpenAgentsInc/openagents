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
    build_tassadar_post_article_canonical_route_universal_substrate_gate_report,
    build_tassadar_post_article_universality_bridge_contract_report,
    build_tassadar_post_article_universality_portability_minimality_matrix_report,
    build_tassadar_universality_verdict_split_report,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
    TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus,
    TassadarPostArticleUniversalityBridgeContractReport,
    TassadarPostArticleUniversalityBridgeContractReportError,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
    TassadarUniversalityVerdictLevel, TassadarUniversalityVerdictSplitReport,
    TassadarUniversalityVerdictSplitReportError, TassadarUniversalityVerdictStatus,
    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
    TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_rebased_universality_verdict_split_report.json";
pub const TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-rebased-universality-verdict-split.sh";

const TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_turing_completeness_closeout_summary.json";
const TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_post_article_universality_served_conformance_envelope.json";
const PLUGIN_AND_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleRebasedUniversalityVerdictSplitStatus {
    TheoryGreenOperatorGreenServedSuppressed,
    Incomplete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleRebasedUniversalitySupportingMaterialClass {
    ProofCarrying,
    ObservationalContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRebasedUniversalitySupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleRebasedUniversalitySupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRebasedUniversalityVerdictRow {
    pub verdict_level: TassadarUniversalityVerdictLevel,
    pub verdict_status: TassadarUniversalityVerdictStatus,
    pub bound_machine_identity_id: String,
    pub canonical_route_id: String,
    pub allowed_statement: String,
    pub source_refs: Vec<String>,
    pub route_constraint_ids: Vec<String>,
    pub allowed_profile_ids: Vec<String>,
    pub blocked_by: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRebasedUniversalityValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleRebasedUniversalityVerdictSplitReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub historical_verdict_split_report_ref: String,
    pub turing_completeness_closeout_summary_ref: String,
    pub universal_substrate_gate_report_ref: String,
    pub portability_minimality_matrix_report_ref: String,
    pub bridge_contract_report_ref: String,
    pub served_conformance_envelope_ref: String,
    pub machine_identity_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub continuation_contract_id: String,
    pub current_served_internal_compute_profile_id: String,
    pub supporting_material_rows: Vec<TassadarPostArticleRebasedUniversalitySupportingMaterialRow>,
    pub verdict_rows: Vec<TassadarPostArticleRebasedUniversalityVerdictRow>,
    pub theory_green: bool,
    pub operator_green: bool,
    pub served_green: bool,
    pub verdict_split_status: TassadarPostArticleRebasedUniversalityVerdictSplitStatus,
    pub verdict_split_green: bool,
    pub deferred_issue_ids: Vec<String>,
    pub rebase_claim_allowed: bool,
    pub plugin_capability_claim_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub validation_rows: Vec<TassadarPostArticleRebasedUniversalityValidationRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleRebasedUniversalityVerdictSplitReportError {
    #[error(transparent)]
    HistoricalVerdictSplit(#[from] TassadarUniversalityVerdictSplitReportError),
    #[error(transparent)]
    UniversalSubstrateGate(
        #[from] TassadarPostArticleCanonicalRouteUniversalSubstrateGateReportError,
    ),
    #[error(transparent)]
    PortabilityMatrix(
        #[from] TassadarPostArticleUniversalityPortabilityMinimalityMatrixReportError,
    ),
    #[error(transparent)]
    Bridge(#[from] TassadarPostArticleUniversalityBridgeContractReportError),
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarTuringCompletenessCloseoutSummaryInput {
    report_id: String,
    allowed_statement: String,
    served_blocked_by: Vec<String>,
    explicit_non_implications: Vec<String>,
    report_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarPostArticleUniversalityServedConformanceEnvelopeInput {
    publication_id: String,
    current_served_internal_compute_profile_id: String,
    canonical_route_id: String,
    served_suppression_boundary_preserved: bool,
    served_public_universality_allowed: bool,
    fail_closed_condition_ids: Vec<String>,
    publication_digest: String,
}

pub fn build_tassadar_post_article_rebased_universality_verdict_split_report() -> Result<
    TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    TassadarPostArticleRebasedUniversalityVerdictSplitReportError,
> {
    let historical_verdict_split = build_tassadar_universality_verdict_split_report()?;
    let universal_substrate_gate =
        build_tassadar_post_article_canonical_route_universal_substrate_gate_report()?;
    let portability_matrix =
        build_tassadar_post_article_universality_portability_minimality_matrix_report()?;
    let bridge = build_tassadar_post_article_universality_bridge_contract_report()?;
    let closeout_summary: TassadarTuringCompletenessCloseoutSummaryInput = read_artifact(
        TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF_LOCAL,
        "turing_completeness_closeout_summary",
    )?;
    let served_conformance_envelope: TassadarPostArticleUniversalityServedConformanceEnvelopeInput =
        read_artifact(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF_LOCAL,
            "post_article_universality_served_conformance_envelope",
        )?;
    Ok(build_report_from_inputs(
        historical_verdict_split,
        closeout_summary,
        universal_substrate_gate,
        portability_matrix,
        bridge,
        served_conformance_envelope,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    historical_verdict_split: TassadarUniversalityVerdictSplitReport,
    closeout_summary: TassadarTuringCompletenessCloseoutSummaryInput,
    universal_substrate_gate: TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    portability_matrix: TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    bridge: TassadarPostArticleUniversalityBridgeContractReport,
    served_conformance_envelope: TassadarPostArticleUniversalityServedConformanceEnvelopeInput,
) -> TassadarPostArticleRebasedUniversalityVerdictSplitReport {
    let supporting_material_rows = build_supporting_material_rows(
        &historical_verdict_split,
        &closeout_summary,
        &universal_substrate_gate,
        &portability_matrix,
        &bridge,
        &served_conformance_envelope,
    );

    let theory_green = historical_verdict_split.theory_green
        && universal_substrate_gate.gate_status
            == TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus::Green
        && universal_substrate_gate.universal_substrate_gate_allowed
        && portability_matrix.matrix_green
        && bridge.bridge_contract_green;
    let operator_green = historical_verdict_split.operator_green
        && theory_green
        && portability_matrix.machine_matrix_green
        && portability_matrix.route_classification_green
        && portability_matrix.minimality_green
        && served_conformance_envelope.served_suppression_boundary_preserved;
    let served_green = false;

    let rebase_claim_allowed = theory_green
        && operator_green
        && !served_green
        && closeout_summary
            .explicit_non_implications
            .iter()
            .any(|id| id == "public universality publication");
    let plugin_capability_claim_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let verdict_rows = build_verdict_rows(
        &historical_verdict_split,
        &bridge,
        &served_conformance_envelope,
        theory_green,
        operator_green,
        served_green,
    );
    let validation_rows = build_validation_rows(
        &universal_substrate_gate,
        &portability_matrix,
        &bridge,
        &served_conformance_envelope,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
    );
    let proof_rows_green = supporting_material_rows
        .iter()
        .filter(|row| {
            row.material_class
                == TassadarPostArticleRebasedUniversalitySupportingMaterialClass::ProofCarrying
        })
        .all(|row| row.satisfied);
    let validation_green = validation_rows.iter().all(|row| row.green);
    let verdict_split_green =
        proof_rows_green && validation_green && theory_green && operator_green && !served_green;
    let verdict_split_status = if verdict_split_green {
        TassadarPostArticleRebasedUniversalityVerdictSplitStatus::TheoryGreenOperatorGreenServedSuppressed
    } else {
        TassadarPostArticleRebasedUniversalityVerdictSplitStatus::Incomplete
    };
    let deferred_issue_ids = Vec::new();

    let mut report = TassadarPostArticleRebasedUniversalityVerdictSplitReport {
        schema_version: 1,
        report_id: String::from("tassadar.post_article_rebased_universality_verdict_split.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_CHECKER_REF,
        ),
        historical_verdict_split_report_ref: String::from(
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
        ),
        turing_completeness_closeout_summary_ref: String::from(
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF_LOCAL,
        ),
        universal_substrate_gate_report_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
        ),
        portability_minimality_matrix_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
        ),
        bridge_contract_report_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
        ),
        served_conformance_envelope_ref: String::from(
            TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF_LOCAL,
        ),
        machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
        canonical_model_id: bridge.bridge_machine_identity.canonical_model_id.clone(),
        canonical_weight_artifact_id: bridge
            .bridge_machine_identity
            .canonical_weight_artifact_id
            .clone(),
        canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
        canonical_route_descriptor_digest: bridge
            .bridge_machine_identity
            .canonical_route_descriptor_digest
            .clone(),
        continuation_contract_id: bridge.bridge_machine_identity.continuation_contract_id.clone(),
        current_served_internal_compute_profile_id: served_conformance_envelope
            .current_served_internal_compute_profile_id
            .clone(),
        supporting_material_rows,
        verdict_rows,
        theory_green,
        operator_green,
        served_green,
        verdict_split_status,
        verdict_split_green,
        deferred_issue_ids,
        rebase_claim_allowed,
        plugin_capability_claim_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        validation_rows,
        claim_boundary: String::from(
            "this report reissues the theory/operator/served universality verdict split on the post-`TAS-186` canonical machine identity. It binds the historical verdict split to the canonical owned route, the canonical weight artifact, the continuation contract, the portability/minimality matrix, and the served conformance envelope. It allows the rebased theory/operator claim only for the canonical route and still does not imply weighted plugin control, served/public universality, or arbitrary software capability.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article rebased universality verdict split keeps supporting_materials={}/{}, validation_rows={}/{}, verdict_split_status={:?}, rebase_claim_allowed={}, and served_green={}.",
        report
            .supporting_material_rows
            .iter()
            .filter(|row| row.satisfied)
            .count(),
        report.supporting_material_rows.len(),
        report.validation_rows.iter().filter(|row| row.green).count(),
        report.validation_rows.len(),
        report.verdict_split_status,
        report.rebase_claim_allowed,
        report.served_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_rebased_universality_verdict_split_report|",
        &report,
    );
    report
}

fn build_supporting_material_rows(
    historical_verdict_split: &TassadarUniversalityVerdictSplitReport,
    closeout_summary: &TassadarTuringCompletenessCloseoutSummaryInput,
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    portability_matrix: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    served_conformance_envelope: &TassadarPostArticleUniversalityServedConformanceEnvelopeInput,
) -> Vec<TassadarPostArticleRebasedUniversalitySupportingMaterialRow> {
    vec![
        supporting_material_row(
            "historical_verdict_split",
            TassadarPostArticleRebasedUniversalitySupportingMaterialClass::ProofCarrying,
            historical_verdict_split.theory_green
                && historical_verdict_split.operator_green
                && !historical_verdict_split.served_green,
            TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            Some(historical_verdict_split.report_id.clone()),
            Some(historical_verdict_split.report_digest.clone()),
            "the historical theory/operator/served split must stay green/suppressed in the old posture before it can be rebound to the canonical route.",
        ),
        supporting_material_row(
            "historical_closeout_summary",
            TassadarPostArticleRebasedUniversalitySupportingMaterialClass::ProofCarrying,
            closeout_summary
                .explicit_non_implications
                .iter()
                .any(|id| id == "public universality publication")
                && !closeout_summary.served_blocked_by.is_empty(),
            TASSADAR_TURING_COMPLETENESS_CLOSEOUT_SUMMARY_REF_LOCAL,
            Some(closeout_summary.report_id.clone()),
            Some(closeout_summary.report_digest.clone()),
            "the historical closeout summary must keep served blockers and non-implications explicit so the rebased split preserves the old public boundary.",
        ),
        supporting_material_row(
            "post_article_bridge_contract",
            TassadarPostArticleRebasedUniversalitySupportingMaterialClass::ProofCarrying,
            bridge.bridge_contract_green,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF,
            Some(bridge.report_id.clone()),
            Some(bridge.report_digest.clone()),
            "the bridge contract must stay green so the rebased split binds to one canonical machine identity instead of a mixed historical reconstruction.",
        ),
        supporting_material_row(
            "canonical_route_universal_substrate_gate",
            TassadarPostArticleRebasedUniversalitySupportingMaterialClass::ProofCarrying,
            universal_substrate_gate.gate_status
                == TassadarPostArticleCanonicalRouteUniversalSubstrateGateStatus::Green
                && universal_substrate_gate.universal_substrate_gate_allowed,
            TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            Some(universal_substrate_gate.report_id.clone()),
            Some(universal_substrate_gate.report_digest.clone()),
            "the canonical-route universal-substrate gate must stay green before the rebased verdict split can publish theory/operator truth on the canonical route.",
        ),
        supporting_material_row(
            "portability_minimality_matrix",
            TassadarPostArticleRebasedUniversalitySupportingMaterialClass::ProofCarrying,
            portability_matrix.matrix_green
                && portability_matrix.served_suppression_boundary_preserved
                && portability_matrix.served_conformance_envelope_defined,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
            Some(portability_matrix.report_id.clone()),
            Some(portability_matrix.report_digest.clone()),
            "the portability/minimality matrix must stay green so theory/operator truth remains bound to the declared CPU machine matrix and explicit route-carrier classification.",
        ),
        supporting_material_row(
            "served_conformance_envelope",
            TassadarPostArticleRebasedUniversalitySupportingMaterialClass::ProofCarrying,
            served_conformance_envelope.current_served_internal_compute_profile_id
                == "tassadar.internal_compute.article_closeout.v1"
                && served_conformance_envelope.served_suppression_boundary_preserved
                && !served_conformance_envelope.served_public_universality_allowed,
            TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF_LOCAL,
            Some(served_conformance_envelope.publication_id.clone()),
            Some(served_conformance_envelope.publication_digest.clone()),
            "the served conformance envelope must stay fail-closed so the rebased split can keep theory/operator truth separate from served/public universality.",
        ),
        supporting_material_row(
            "plugin_audit_context",
            TassadarPostArticleRebasedUniversalitySupportingMaterialClass::ObservationalContext,
            true,
            PLUGIN_AND_TURING_AUDIT_REF,
            None,
            None,
            "the plugin/turing audit remains observational context here and explains why the rebased verdict split must not over-read future plugin capability.",
        ),
    ]
}

fn build_verdict_rows(
    historical_verdict_split: &TassadarUniversalityVerdictSplitReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    served_conformance_envelope: &TassadarPostArticleUniversalityServedConformanceEnvelopeInput,
    theory_green: bool,
    operator_green: bool,
    served_green: bool,
) -> Vec<TassadarPostArticleRebasedUniversalityVerdictRow> {
    let theory_row = historical_verdict_split
        .verdict_rows
        .iter()
        .find(|row| row.verdict_level == TassadarUniversalityVerdictLevel::Theory)
        .expect("historical theory row should exist");
    let operator_row = historical_verdict_split
        .verdict_rows
        .iter()
        .find(|row| row.verdict_level == TassadarUniversalityVerdictLevel::Operator)
        .expect("historical operator row should exist");
    let served_row = historical_verdict_split
        .verdict_rows
        .iter()
        .find(|row| row.verdict_level == TassadarUniversalityVerdictLevel::Served)
        .expect("historical served row should exist");

    vec![
        TassadarPostArticleRebasedUniversalityVerdictRow {
            verdict_level: TassadarUniversalityVerdictLevel::Theory,
            verdict_status: if theory_green {
                TassadarUniversalityVerdictStatus::Green
            } else {
                TassadarUniversalityVerdictStatus::Suppressed
            },
            bound_machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
            canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
            allowed_statement: format!(
                "Psionic/Tassadar can honestly say that the post-`TAS-186` canonical route on machine_identity_id=`{}` carries the declared `TCM.v1` universal substrate plus explicit universal-machine witness constructions under the declared checkpoint-and-resume semantics.",
                bridge.bridge_machine_identity.machine_identity_id,
            ),
            source_refs: vec![
                String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
            ],
            route_constraint_ids: vec![
                String::from("theory_has_no_served_route_claim"),
                String::from("canonical_machine_identity_bound"),
                String::from("canonical_route_universal_substrate_gate_green"),
                String::from("canonical_portability_matrix_bound"),
            ],
            allowed_profile_ids: theory_row.allowed_profile_ids.clone(),
            blocked_by: Vec::new(),
            detail: format!(
                "{} Rebased theory truth is now bound to canonical_route_id=`{}` and canonical_route_descriptor_digest=`{}`.",
                theory_row.detail,
                bridge.bridge_machine_identity.canonical_route_id,
                bridge.bridge_machine_identity.canonical_route_descriptor_digest,
            ),
        },
        TassadarPostArticleRebasedUniversalityVerdictRow {
            verdict_level: TassadarUniversalityVerdictLevel::Operator,
            verdict_status: if operator_green {
                TassadarUniversalityVerdictStatus::Green
            } else {
                TassadarUniversalityVerdictStatus::Suppressed
            },
            bound_machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
            canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
            allowed_statement: format!(
                "Psionic/Tassadar can honestly say that operators have one bounded universality-capable lane on the post-`TAS-186` canonical route under named session-process, spill/tape, and process-object envelopes with exact checkpoint-and-replay evidence.",
            ),
            source_refs: vec![
                String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            route_constraint_ids: vec![
                String::from("operator_named_profile_routes_only"),
                String::from("operator_checkpoint_resume_required"),
                String::from("operator_spill_tape_extension_required"),
                String::from("canonical_machine_identity_bound"),
                String::from("operator_canonical_portability_matrix_bound"),
                String::from("operator_served_boundary_not_widened"),
            ],
            allowed_profile_ids: operator_row.allowed_profile_ids.clone(),
            blocked_by: Vec::new(),
            detail: format!(
                "{} Rebased operator truth is now frozen on machine_identity_id=`{}` with canonical_route_id=`{}`.",
                operator_row.detail,
                bridge.bridge_machine_identity.machine_identity_id,
                bridge.bridge_machine_identity.canonical_route_id,
            ),
        },
        TassadarPostArticleRebasedUniversalityVerdictRow {
            verdict_level: TassadarUniversalityVerdictLevel::Served,
            verdict_status: if served_green {
                TassadarUniversalityVerdictStatus::Green
            } else {
                TassadarUniversalityVerdictStatus::Suppressed
            },
            bound_machine_identity_id: bridge.bridge_machine_identity.machine_identity_id.clone(),
            canonical_route_id: bridge.bridge_machine_identity.canonical_route_id.clone(),
            allowed_statement: format!(
                "Psionic/Tassadar does not yet expose a served/public universality lane on the post-`TAS-186` canonical route; served posture remains the narrower `{}` lane inside the declared conformance envelope.",
                served_conformance_envelope.current_served_internal_compute_profile_id,
            ),
            source_refs: vec![
                String::from(TASSADAR_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF_LOCAL),
            ],
            route_constraint_ids: vec![
                String::from("served_universality_route_publication_suppressed"),
                String::from("served_universality_profile_not_selected"),
                String::from("served_conformance_envelope_fail_closed"),
            ],
            allowed_profile_ids: served_row.allowed_profile_ids.clone(),
            blocked_by: served_row.blocked_by.clone(),
            detail: format!(
                "{} The rebased served row remains suppressed because the served conformance envelope is fail-closed and served/public universality is still out of scope.",
                served_row.detail,
            ),
        },
    ]
}

#[allow(clippy::too_many_arguments)]
fn build_validation_rows(
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    portability_matrix: &TassadarPostArticleUniversalityPortabilityMinimalityMatrixReport,
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    served_conformance_envelope: &TassadarPostArticleUniversalityServedConformanceEnvelopeInput,
    rebase_claim_allowed: bool,
    plugin_capability_claim_allowed: bool,
    served_public_universality_allowed: bool,
    arbitrary_software_capability_allowed: bool,
) -> Vec<TassadarPostArticleRebasedUniversalityValidationRow> {
    vec![
        validation_row(
            "helper_substitution_quarantined",
            gate_validation_green(universal_substrate_gate, "helper_substitution_quarantined"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "helper substitution remains quarantined, so the rebased verdict split cannot inherit canonical-route truth through hidden host execution.",
        ),
        validation_row(
            "route_drift_rejected",
            gate_validation_green(universal_substrate_gate, "route_drift_rejected")
                && portability_matrix
                    .validation_rows
                    .iter()
                    .find(|row| row.validation_id == "route_drift_rejected")
                    .is_some_and(|row| row.green),
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
            ],
            "route drift remains rejected, so the rebased verdict split stays attached to one canonical route id and one descriptor digest.",
        ),
        validation_row(
            "continuation_abuse_quarantined",
            gate_validation_green(universal_substrate_gate, "continuation_abuse_quarantined"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "continuation abuse remains quarantined, so operator truth still depends on explicit checkpoint-and-resume semantics rather than hidden control transfer.",
        ),
        validation_row(
            "semantic_drift_blocked",
            gate_validation_green(universal_substrate_gate, "semantic_drift_blocked"),
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "semantic drift remains blocked, so the rebased verdict split does not treat portability or publication as permission to change the compute semantics.",
        ),
        validation_row(
            "historical_artifacts_preserved_without_rewrite",
            bridge_validation_green(bridge, "historical_artifacts_preserved_without_rewrite"),
            vec![String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF)],
            "historical universality artifacts remain preserved without rewrite, so the rebased split is explicit rebinding rather than narrative replacement.",
        ),
        validation_row(
            "canonical_machine_matrix_bound",
            portability_matrix.matrix_green
                && portability_matrix.machine_identity_id
                    == bridge.bridge_machine_identity.machine_identity_id
                && portability_matrix.canonical_route_id
                    == bridge.bridge_machine_identity.canonical_route_id,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_UNIVERSALITY_PORTABILITY_MINIMALITY_MATRIX_REPORT_REF,
                ),
                String::from(TASSADAR_POST_ARTICLE_UNIVERSALITY_BRIDGE_CONTRACT_REPORT_REF),
            ],
            "the rebased verdict split stays bound to the declared canonical machine identity and machine matrix rather than inheriting truth from an unspecified host/runtime mix.",
        ),
        validation_row(
            "served_conformance_envelope_bound",
            served_conformance_envelope.served_suppression_boundary_preserved
                && !served_conformance_envelope.served_public_universality_allowed
                && served_conformance_envelope.current_served_internal_compute_profile_id
                    == "tassadar.internal_compute.article_closeout.v1",
            vec![String::from(
                TASSADAR_POST_ARTICLE_UNIVERSALITY_SERVED_CONFORMANCE_ENVELOPE_REF_LOCAL,
            )],
            "the served conformance envelope stays bound and fail-closed, so served posture remains the narrower article-closeout lane instead of silently widening into served universality.",
        ),
        validation_row(
            "overclaim_posture_explicit",
            gate_validation_green(universal_substrate_gate, "overclaim_posture_explicit")
                && rebase_claim_allowed
                && !plugin_capability_claim_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![String::from(
                TASSADAR_POST_ARTICLE_CANONICAL_ROUTE_UNIVERSAL_SUBSTRATE_GATE_REPORT_REF,
            )],
            "the rebased theory/operator split is now allowed, but plugin capability, served/public universality, and arbitrary software capability remain explicitly out of scope.",
        ),
    ]
}

fn supporting_material_row(
    material_id: &str,
    material_class: TassadarPostArticleRebasedUniversalitySupportingMaterialClass,
    satisfied: bool,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleRebasedUniversalitySupportingMaterialRow {
    TassadarPostArticleRebasedUniversalitySupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied,
        source_ref: String::from(source_ref),
        source_artifact_id,
        source_artifact_digest,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleRebasedUniversalityValidationRow {
    TassadarPostArticleRebasedUniversalityValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

fn gate_validation_green(
    universal_substrate_gate: &TassadarPostArticleCanonicalRouteUniversalSubstrateGateReport,
    validation_id: &str,
) -> bool {
    universal_substrate_gate
        .validation_rows
        .iter()
        .find(|row| row.validation_id == validation_id)
        .is_some_and(|row| row.green)
}

fn bridge_validation_green(
    bridge: &TassadarPostArticleUniversalityBridgeContractReport,
    validation_id: &str,
) -> bool {
    bridge
        .validation_rows
        .iter()
        .find(|row| row.validation_id == validation_id)
        .is_some_and(|row| row.green)
}

#[must_use]
pub fn tassadar_post_article_rebased_universality_verdict_split_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF)
}

pub fn write_tassadar_post_article_rebased_universality_verdict_split_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleRebasedUniversalityVerdictSplitReport,
    TassadarPostArticleRebasedUniversalityVerdictSplitReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleRebasedUniversalityVerdictSplitReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_rebased_universality_verdict_split_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictSplitReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root should resolve from psionic-eval crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_artifact<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarPostArticleRebasedUniversalityVerdictSplitReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictSplitReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictSplitReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleRebasedUniversalityVerdictSplitReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictSplitReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleRebasedUniversalityVerdictSplitReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_rebased_universality_verdict_split_report, read_json,
        tassadar_post_article_rebased_universality_verdict_split_report_path,
        write_tassadar_post_article_rebased_universality_verdict_split_report,
        TassadarPostArticleRebasedUniversalityVerdictSplitReport,
        TassadarPostArticleRebasedUniversalityVerdictSplitStatus,
        TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
    };
    use crate::TassadarUniversalityVerdictLevel;
    use tempfile::tempdir;

    #[test]
    fn post_article_rebased_universality_verdict_split_turns_green_when_prereqs_hold() {
        let report = build_tassadar_post_article_rebased_universality_verdict_split_report()
            .expect("report");

        assert_eq!(
            report.verdict_split_status,
            TassadarPostArticleRebasedUniversalityVerdictSplitStatus::TheoryGreenOperatorGreenServedSuppressed
        );
        assert!(report.verdict_split_green);
        assert!(report.theory_green);
        assert!(report.operator_green);
        assert!(!report.served_green);
        assert_eq!(
            report.current_served_internal_compute_profile_id,
            "tassadar.internal_compute.article_closeout.v1"
        );
        assert_eq!(report.supporting_material_rows.len(), 7);
        assert_eq!(report.validation_rows.len(), 8);
        assert!(report.deferred_issue_ids.is_empty());
        assert!(report.rebase_claim_allowed);
        assert!(!report.plugin_capability_claim_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert!(report.verdict_rows.iter().any(|row| row.verdict_level
            == TassadarUniversalityVerdictLevel::Served
            && !row.blocked_by.is_empty()));
    }

    #[test]
    fn post_article_rebased_universality_verdict_split_matches_committed_truth() {
        let generated = build_tassadar_post_article_rebased_universality_verdict_split_report()
            .expect("report");
        let committed: TassadarPostArticleRebasedUniversalityVerdictSplitReport =
            read_json(tassadar_post_article_rebased_universality_verdict_split_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_POST_ARTICLE_REBASED_UNIVERSALITY_VERDICT_SPLIT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_rebased_universality_verdict_split_report.json"
        );
    }

    #[test]
    fn write_post_article_rebased_universality_verdict_split_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_rebased_universality_verdict_split_report.json");
        let written =
            write_tassadar_post_article_rebased_universality_verdict_split_report(&output_path)
                .expect("write report");
        let persisted: TassadarPostArticleRebasedUniversalityVerdictSplitReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read report"))
                .expect("decode report");
        assert_eq!(written, persisted);
    }
}
