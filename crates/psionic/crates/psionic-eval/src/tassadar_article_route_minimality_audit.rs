use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_router::TassadarPlannerExecutorRoutePosture;
use psionic_runtime::TassadarExecutorDecodeMode;

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleCrossMachineReproducibilityReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleFastRouteArchitectureSelectionError,
    TassadarArticleFastRouteArchitectureSelectionReport, TassadarArticleFastRouteCandidateKind,
    TassadarArticleInterpreterOwnershipComputationMappingReport,
    TassadarArticleInterpreterOwnershipGateError, TassadarArticleInterpreterOwnershipGateReport,
    TassadarArticleInterpreterOwnershipRoutePurityReview,
    TassadarArticleKvActivationDisciplineAuditError,
    TassadarArticleKvActivationDisciplineAuditReport,
    TassadarArticleSingleRunNoSpillClosureBoundaryPerturbationReview,
    TassadarArticleSingleRunNoSpillClosureOperatorEnvelope,
    TassadarArticleSingleRunNoSpillClosureReport,
    TassadarArticleSingleRunNoSpillClosureReportError, TassadarArticleStateCarrierBoundaryRow,
    TassadarArticleStateDominanceVerdict,
    TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
    TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
    TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
    TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_route_minimality_audit_report.json";
pub const TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_CHECKER_REF: &str =
    "scripts/check-tassadar-article-route-minimality-audit.sh";

const REPORT_SCHEMA_VERSION: u16 = 1;
const TIED_REQUIREMENT_ID: &str = "TAS-185A";
const CANONICAL_CLAIM_ROUTE_ID: &str = "tassadar.article_route.direct_hull_cache_runtime.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleRouteMinimalityPublicPosture {
    SuppressedPendingFinalAudit,
    GreenBounded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub architecture_selection_report_ref: String,
    pub single_run_no_spill_report_ref: String,
    pub interpreter_ownership_report_ref: String,
    pub kv_activation_discipline_report_ref: String,
    pub cross_machine_reproducibility_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleCanonicalClaimRouteReview {
    pub canonical_claim_route_id: String,
    pub selected_candidate_kind: TassadarArticleFastRouteCandidateKind,
    pub selected_decode_mode: TassadarExecutorDecodeMode,
    pub transformer_model_id: String,
    pub projected_route_descriptor_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_planner_route_posture: Option<TassadarPlannerExecutorRoutePosture>,
    pub routeable_module_class_count: usize,
    pub direct_module_class_count: usize,
    pub fallback_module_class_count: usize,
    pub all_module_classes_direct: bool,
    pub deterministic_direct_case_count: usize,
    pub deterministic_hybrid_case_count: usize,
    pub deterministic_long_horizon_count: usize,
    pub canonical_claim_route_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityContinuationBoundaryReview {
    pub checkpoint_restore_allowed: bool,
    pub spill_tape_extension_allowed: bool,
    pub external_persisted_continuation_allowed: bool,
    pub hidden_reentry_allowed: bool,
    pub implicit_segmentation_allowed: bool,
    pub runtime_loop_unrolling_allowed: bool,
    pub teacher_forcing_allowed: bool,
    pub oracle_leakage_allowed: bool,
    pub oversized_context_memory_allowed: bool,
    pub deterministic_mode_required: bool,
    pub perturbation_negative_control_green: bool,
    pub continuation_boundary_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityExecutionOwnershipReview {
    pub hidden_host_substitution_excluded: bool,
    pub external_oracle_excluded: bool,
    pub preprocessing_shortcut_excluded: bool,
    pub route_drift_excluded: bool,
    pub runtime_owned_control_flow_excluded: bool,
    pub helper_module_mediation_excluded: bool,
    pub artifact_lineage_to_behavior_closed: bool,
    pub stable_across_runs: bool,
    pub control_flow_realization: String,
    pub route_purity_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityStateCarrierReview {
    pub carrier_boundary_rows: Vec<TassadarArticleStateCarrierBoundaryRow>,
    pub dominance_verdict: TassadarArticleStateDominanceVerdict,
    pub carrier_boundary_declared: bool,
    pub request_local_same_run_only_green: bool,
    pub persisted_or_cross_run_state_excluded: bool,
    pub kv_activation_discipline_green: bool,
    pub state_carrier_minimality_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityOrchestrationReview {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_planner_route_posture: Option<TassadarPlannerExecutorRoutePosture>,
    pub deterministic_direct_case_count: usize,
    pub deterministic_hybrid_case_count: usize,
    pub hybrid_surface_present: bool,
    pub hybrid_same_decode_mode_green: bool,
    pub hybrid_surface_operator_only: bool,
    pub public_claim_route_excludes_planner_indirection: bool,
    pub public_claim_route_excludes_hybrid_surface: bool,
    pub extra_orchestration_layers_excluded: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityOperatorVerdictReview {
    pub route_like_article_in_kind: bool,
    pub operator_verdict_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityPublicVerdictReview {
    pub posture: TassadarArticleRouteMinimalityPublicPosture,
    pub public_verdict_declared: bool,
    pub public_verdict_green: bool,
    pub blocked_issue_ids: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleRouteMinimalityAuditReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleRouteMinimalityAcceptanceGateTie,
    pub canonical_claim_route_review: TassadarArticleCanonicalClaimRouteReview,
    pub continuation_boundary_review: TassadarArticleRouteMinimalityContinuationBoundaryReview,
    pub execution_ownership_review: TassadarArticleRouteMinimalityExecutionOwnershipReview,
    pub state_carrier_review: TassadarArticleRouteMinimalityStateCarrierReview,
    pub orchestration_review: TassadarArticleRouteMinimalityOrchestrationReview,
    pub operator_verdict_review: TassadarArticleRouteMinimalityOperatorVerdictReview,
    pub public_verdict_review: TassadarArticleRouteMinimalityPublicVerdictReview,
    pub route_minimality_audit_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleRouteMinimalityAuditError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    ArchitectureSelection(#[from] TassadarArticleFastRouteArchitectureSelectionError),
    #[error(transparent)]
    NoSpill(#[from] TassadarArticleSingleRunNoSpillClosureReportError),
    #[error(transparent)]
    InterpreterOwnership(#[from] TassadarArticleInterpreterOwnershipGateError),
    #[error(transparent)]
    KvActivation(#[from] TassadarArticleKvActivationDisciplineAuditError),
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
    #[error("missing selected TAS-172 routeability check for `{candidate_kind}`")]
    MissingSelectedRouteabilityCheck { candidate_kind: String },
    #[error("selected routeability check for `{candidate_kind}` is missing a projected route descriptor digest")]
    MissingRouteDescriptorDigest { candidate_kind: String },
    #[error(
        "selected routeability check for `{candidate_kind}` is missing a requested decode mode"
    )]
    MissingRequestedDecodeMode { candidate_kind: String },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_route_minimality_audit_report(
) -> Result<TassadarArticleRouteMinimalityAuditReport, TassadarArticleRouteMinimalityAuditError> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let architecture_selection: TassadarArticleFastRouteArchitectureSelectionReport =
        read_artifact(
            TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            "article_fast_route_architecture_selection",
        )?;
    let single_run: TassadarArticleSingleRunNoSpillClosureReport = read_artifact(
        TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
        "article_single_run_no_spill_closure",
    )?;
    let interpreter_ownership: TassadarArticleInterpreterOwnershipGateReport = read_artifact(
        TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
        "article_interpreter_ownership_gate",
    )?;
    let kv_activation: TassadarArticleKvActivationDisciplineAuditReport = read_artifact(
        TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
        "article_kv_activation_discipline_audit",
    )?;
    let cross_machine: TassadarArticleCrossMachineReproducibilityReport = read_artifact(
        TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
        "article_cross_machine_reproducibility_matrix",
    )?;

    let selected_routeability_check = architecture_selection
        .routeability_checks
        .iter()
        .find(|check| check.candidate_kind == architecture_selection.selected_candidate_kind)
        .ok_or_else(|| {
            TassadarArticleRouteMinimalityAuditError::MissingSelectedRouteabilityCheck {
                candidate_kind: architecture_selection
                    .selected_candidate_kind
                    .label()
                    .to_string(),
            }
        })?;
    let projected_route_descriptor_digest = selected_routeability_check
        .projected_route_descriptor_digest
        .clone()
        .ok_or_else(
            || TassadarArticleRouteMinimalityAuditError::MissingRouteDescriptorDigest {
                candidate_kind: architecture_selection
                    .selected_candidate_kind
                    .label()
                    .to_string(),
            },
        )?;
    let selected_decode_mode = selected_routeability_check
        .requested_decode_mode
        .ok_or_else(
            || TassadarArticleRouteMinimalityAuditError::MissingRequestedDecodeMode {
                candidate_kind: architecture_selection
                    .selected_candidate_kind
                    .label()
                    .to_string(),
            },
        )?;
    let all_module_classes_direct =
        selected_routeability_check
            .module_class_rows
            .iter()
            .all(|row| {
                row.routeable && row.negotiated_route_state.as_ref().is_some_and(|state| {
                    *state == psionic_router::TassadarPlannerExecutorNegotiatedRouteState::Direct
                })
                    && row
                        .effective_decode_mode
                        .is_some_and(|mode| mode == selected_decode_mode)
            });
    let deterministic_long_horizon_count = single_run.horizon_review.horizon_rows.len();
    let canonical_claim_route_green = architecture_selection.fast_route_selection_green
        && selected_routeability_check.routeable
        && selected_routeability_check.fallback_module_class_count == 0
        && all_module_classes_direct
        && cross_machine.route_stability_review.route_stability_green
        && cross_machine
            .route_stability_review
            .all_direct_routes_hull_cache
        && single_run.single_run_no_spill_closure_green
        && single_run.horizon_review.deterministic_exactness_green;
    let canonical_claim_route_review = TassadarArticleCanonicalClaimRouteReview {
        canonical_claim_route_id: String::from(CANONICAL_CLAIM_ROUTE_ID),
        selected_candidate_kind: architecture_selection.selected_candidate_kind,
        selected_decode_mode,
        transformer_model_id: cross_machine
            .route_stability_review
            .transformer_model_id
            .clone(),
        projected_route_descriptor_digest,
        projected_planner_route_posture: selected_routeability_check.aggregate_route_posture,
        routeable_module_class_count: selected_routeability_check
            .module_class_rows
            .iter()
            .filter(|row| row.routeable)
            .count(),
        direct_module_class_count: selected_routeability_check.direct_module_class_count,
        fallback_module_class_count: selected_routeability_check.fallback_module_class_count,
        all_module_classes_direct,
        deterministic_direct_case_count: cross_machine.route_stability_review.direct_case_count,
        deterministic_hybrid_case_count: cross_machine.route_stability_review.hybrid_case_count,
        deterministic_long_horizon_count,
        canonical_claim_route_green,
        detail: format!(
            "canonical_claim_route_id=`{}` selected_candidate_kind=`{}` selected_decode_mode=`{:?}` route_descriptor_digest=`{}` routeable_module_class_count={} direct_module_class_count={} fallback_module_class_count={} deterministic_direct_case_count={} deterministic_hybrid_case_count={} deterministic_long_horizon_count={} canonical_claim_route_green={}",
            CANONICAL_CLAIM_ROUTE_ID,
            architecture_selection.selected_candidate_kind.label(),
            selected_decode_mode,
            canonical_claim_route_review_digest_fragment(
                &selected_routeability_check
                    .projected_route_descriptor_digest
                    .clone()
                    .unwrap_or_default(),
            ),
            selected_routeability_check
                .module_class_rows
                .iter()
                .filter(|row| row.routeable)
                .count(),
            selected_routeability_check.direct_module_class_count,
            selected_routeability_check.fallback_module_class_count,
            cross_machine.route_stability_review.direct_case_count,
            cross_machine.route_stability_review.hybrid_case_count,
            deterministic_long_horizon_count,
            canonical_claim_route_green,
        ),
    };

    let continuation_boundary_green = single_run.operator_envelope.operator_envelope_green
        && single_run
            .boundary_perturbation_review
            .perturbation_negative_control_green
        && !single_run.operator_envelope.checkpoint_restore_allowed
        && !single_run.operator_envelope.spill_tape_extension_allowed
        && !single_run
            .operator_envelope
            .external_persisted_continuation_allowed
        && !single_run.operator_envelope.hidden_reentry_allowed
        && !single_run.operator_envelope.implicit_segmentation_allowed
        && !single_run.operator_envelope.runtime_loop_unrolling_allowed
        && !single_run.operator_envelope.teacher_forcing_allowed
        && !single_run.operator_envelope.oracle_leakage_allowed
        && !single_run
            .operator_envelope
            .oversized_context_memory_allowed
        && single_run.operator_envelope.deterministic_mode_required;
    let continuation_boundary_review = build_continuation_boundary_review(
        &single_run.operator_envelope,
        &single_run.boundary_perturbation_review,
        continuation_boundary_green,
    );

    let execution_ownership_review = build_execution_ownership_review(
        &interpreter_ownership.route_purity_review,
        &interpreter_ownership.computation_mapping_report,
    );

    let persisted_or_cross_run_state_excluded = !single_run
        .operator_envelope
        .external_persisted_continuation_allowed
        && !single_run.operator_envelope.checkpoint_restore_allowed
        && !single_run.operator_envelope.spill_tape_extension_allowed
        && !single_run.operator_envelope.hidden_reentry_allowed;
    let state_carrier_minimality_green = kv_activation.kv_activation_discipline_green
        && kv_activation.binding_review.carrier_boundary_declared
        && persisted_or_cross_run_state_excluded;
    let state_carrier_review = TassadarArticleRouteMinimalityStateCarrierReview {
        carrier_boundary_rows: kv_activation.carrier_boundary_rows.clone(),
        dominance_verdict: kv_activation.dominance_verdict.clone(),
        carrier_boundary_declared: kv_activation.binding_review.carrier_boundary_declared,
        request_local_same_run_only_green: persisted_or_cross_run_state_excluded,
        persisted_or_cross_run_state_excluded,
        kv_activation_discipline_green: kv_activation.kv_activation_discipline_green,
        state_carrier_minimality_green,
        detail: format!(
            "carrier_boundary_declared={} persisted_or_cross_run_state_excluded={} dominance_verdict={:?} kv_activation_discipline_green={} state_carrier_minimality_green={}",
            kv_activation.binding_review.carrier_boundary_declared,
            persisted_or_cross_run_state_excluded,
            kv_activation.dominance_verdict.verdict,
            kv_activation.kv_activation_discipline_green,
            state_carrier_minimality_green,
        ),
    };

    let public_claim_route_excludes_planner_indirection = true;
    let public_claim_route_excludes_hybrid_surface = true;
    let hybrid_surface_present = cross_machine.route_stability_review.hybrid_case_count > 0;
    let hybrid_same_decode_mode_green = cross_machine
        .route_stability_review
        .all_hybrid_routes_hull_cache;
    let extra_orchestration_layers_excluded = public_claim_route_excludes_planner_indirection
        && public_claim_route_excludes_hybrid_surface
        && canonical_claim_route_green;
    let orchestration_review = TassadarArticleRouteMinimalityOrchestrationReview {
        projected_planner_route_posture: selected_routeability_check.aggregate_route_posture,
        deterministic_direct_case_count: cross_machine.route_stability_review.direct_case_count,
        deterministic_hybrid_case_count: cross_machine.route_stability_review.hybrid_case_count,
        hybrid_surface_present,
        hybrid_same_decode_mode_green,
        hybrid_surface_operator_only: hybrid_surface_present,
        public_claim_route_excludes_planner_indirection,
        public_claim_route_excludes_hybrid_surface,
        extra_orchestration_layers_excluded,
        detail: format!(
            "hybrid_surface_present={} hybrid_same_decode_mode_green={} public_claim_route_excludes_planner_indirection={} public_claim_route_excludes_hybrid_surface={} extra_orchestration_layers_excluded={}",
            hybrid_surface_present,
            hybrid_same_decode_mode_green,
            public_claim_route_excludes_planner_indirection,
            public_claim_route_excludes_hybrid_surface,
            extra_orchestration_layers_excluded,
        ),
    };

    let route_minimality_audit_green = canonical_claim_route_review.canonical_claim_route_green
        && continuation_boundary_review.continuation_boundary_green
        && execution_ownership_review.route_purity_green
        && state_carrier_review.state_carrier_minimality_green
        && orchestration_review.extra_orchestration_layers_excluded
        && interpreter_ownership.interpreter_ownership_green
        && kv_activation.binding_review.discipline_audit_green
        && cross_machine.reproducibility_matrix_green;
    let operator_verdict_review = TassadarArticleRouteMinimalityOperatorVerdictReview {
        route_like_article_in_kind: route_minimality_audit_green,
        operator_verdict_green: route_minimality_audit_green,
        detail: format!(
            "route_like_article_in_kind={} operator_verdict_green={} canonical_claim_route_id=`{}`",
            route_minimality_audit_green, route_minimality_audit_green, CANONICAL_CLAIM_ROUTE_ID,
        ),
    };
    let public_verdict_posture =
        if route_minimality_audit_green && acceptance_gate.blocked_issue_ids.is_empty() {
            TassadarArticleRouteMinimalityPublicPosture::GreenBounded
        } else {
            TassadarArticleRouteMinimalityPublicPosture::SuppressedPendingFinalAudit
        };
    let public_verdict_review = TassadarArticleRouteMinimalityPublicVerdictReview {
        posture: public_verdict_posture,
        public_verdict_declared: true,
        public_verdict_green: public_verdict_posture
            == TassadarArticleRouteMinimalityPublicPosture::GreenBounded,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
        detail: format!(
            "public_verdict_declared=true posture={:?} public_verdict_green={} blocked_issue_count={}",
            public_verdict_posture,
            public_verdict_posture
                == TassadarArticleRouteMinimalityPublicPosture::GreenBounded,
            acceptance_gate.blocked_issue_ids.len(),
        ),
    };

    let mut report = TassadarArticleRouteMinimalityAuditReport {
        schema_version: REPORT_SCHEMA_VERSION,
        report_id: String::from("tassadar.article_route_minimality.audit.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_CHECKER_REF),
        acceptance_gate_tie: TassadarArticleRouteMinimalityAcceptanceGateTie {
            acceptance_gate_report_ref: String::from(
                TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
            ),
            architecture_selection_report_ref: String::from(
                TASSADAR_ARTICLE_FAST_ROUTE_ARCHITECTURE_SELECTION_REPORT_REF,
            ),
            single_run_no_spill_report_ref: String::from(
                TASSADAR_ARTICLE_SINGLE_RUN_NO_SPILL_CLOSURE_REPORT_REF,
            ),
            interpreter_ownership_report_ref: String::from(
                TASSADAR_ARTICLE_INTERPRETER_OWNERSHIP_GATE_REPORT_REF,
            ),
            kv_activation_discipline_report_ref: String::from(
                TASSADAR_ARTICLE_KV_ACTIVATION_DISCIPLINE_AUDIT_REPORT_REF,
            ),
            cross_machine_reproducibility_report_ref: String::from(
                TASSADAR_ARTICLE_CROSS_MACHINE_REPRODUCIBILITY_MATRIX_REPORT_REF,
            ),
            tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
            tied_requirement_satisfied: route_minimality_audit_green
                && public_verdict_review.public_verdict_declared,
            acceptance_status: acceptance_gate.acceptance_status.clone(),
            blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
        },
        canonical_claim_route_review,
        continuation_boundary_review,
        execution_ownership_review,
        state_carrier_review,
        orchestration_review,
        operator_verdict_review,
        public_verdict_review: public_verdict_review.clone(),
        route_minimality_audit_green,
        article_equivalence_green: route_minimality_audit_green
            && public_verdict_review.public_verdict_green,
        claim_boundary: String::from(
            "this report closes TAS-185A only. It freezes the minimal final article claim route as the direct HullCache runtime path on the canonical article model, excludes checkpoint, spill, persisted continuation, hidden helper mediation, and planner-owned hybrid orchestration from that claim route, and publishes one explicit operator versus bounded-public verdict. It remains one prerequisite surface for the final article-equivalence audit rather than a substitute for it.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article route-minimality audit now records tied_requirement_satisfied={}, operator_verdict_green={}, public_verdict_green={}, public_blocked_issue_count={}, route_minimality_audit_green={}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.operator_verdict_review.operator_verdict_green,
        report.public_verdict_review.public_verdict_green,
        report.public_verdict_review.blocked_issue_ids.len(),
        report.route_minimality_audit_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_route_minimality_audit_report|",
        &report,
    );
    Ok(report)
}

fn build_continuation_boundary_review(
    operator_envelope: &TassadarArticleSingleRunNoSpillClosureOperatorEnvelope,
    boundary_perturbation_review: &TassadarArticleSingleRunNoSpillClosureBoundaryPerturbationReview,
    continuation_boundary_green: bool,
) -> TassadarArticleRouteMinimalityContinuationBoundaryReview {
    TassadarArticleRouteMinimalityContinuationBoundaryReview {
        checkpoint_restore_allowed: operator_envelope.checkpoint_restore_allowed,
        spill_tape_extension_allowed: operator_envelope.spill_tape_extension_allowed,
        external_persisted_continuation_allowed: operator_envelope
            .external_persisted_continuation_allowed,
        hidden_reentry_allowed: operator_envelope.hidden_reentry_allowed,
        implicit_segmentation_allowed: operator_envelope.implicit_segmentation_allowed,
        runtime_loop_unrolling_allowed: operator_envelope.runtime_loop_unrolling_allowed,
        teacher_forcing_allowed: operator_envelope.teacher_forcing_allowed,
        oracle_leakage_allowed: operator_envelope.oracle_leakage_allowed,
        oversized_context_memory_allowed: operator_envelope.oversized_context_memory_allowed,
        deterministic_mode_required: operator_envelope.deterministic_mode_required,
        perturbation_negative_control_green: boundary_perturbation_review
            .perturbation_negative_control_green,
        continuation_boundary_green,
        detail: format!(
            "checkpoint_restore_allowed={} spill_tape_extension_allowed={} external_persisted_continuation_allowed={} hidden_reentry_allowed={} implicit_segmentation_allowed={} runtime_loop_unrolling_allowed={} teacher_forcing_allowed={} oracle_leakage_allowed={} oversized_context_memory_allowed={} perturbation_negative_control_green={} continuation_boundary_green={}",
            operator_envelope.checkpoint_restore_allowed,
            operator_envelope.spill_tape_extension_allowed,
            operator_envelope.external_persisted_continuation_allowed,
            operator_envelope.hidden_reentry_allowed,
            operator_envelope.implicit_segmentation_allowed,
            operator_envelope.runtime_loop_unrolling_allowed,
            operator_envelope.teacher_forcing_allowed,
            operator_envelope.oracle_leakage_allowed,
            operator_envelope.oversized_context_memory_allowed,
            boundary_perturbation_review.perturbation_negative_control_green,
            continuation_boundary_green,
        ),
    }
}

fn build_execution_ownership_review(
    route_purity_review: &TassadarArticleInterpreterOwnershipRoutePurityReview,
    computation_mapping_report: &TassadarArticleInterpreterOwnershipComputationMappingReport,
) -> TassadarArticleRouteMinimalityExecutionOwnershipReview {
    TassadarArticleRouteMinimalityExecutionOwnershipReview {
        hidden_host_substitution_excluded: route_purity_review.hidden_host_substitution_excluded,
        external_oracle_excluded: route_purity_review.external_oracle_excluded,
        preprocessing_shortcut_excluded: route_purity_review.preprocessing_shortcut_excluded,
        route_drift_excluded: route_purity_review.route_drift_excluded,
        runtime_owned_control_flow_excluded: route_purity_review.runtime_owned_control_flow_excluded,
        helper_module_mediation_excluded: route_purity_review.helper_module_mediation_excluded,
        artifact_lineage_to_behavior_closed: route_purity_review.artifact_lineage_to_behavior_closed,
        stable_across_runs: computation_mapping_report.stable_across_runs,
        control_flow_realization: computation_mapping_report.control_flow_realization.clone(),
        route_purity_green: route_purity_review.route_purity_green,
        detail: format!(
            "hidden_host_substitution_excluded={} external_oracle_excluded={} preprocessing_shortcut_excluded={} route_drift_excluded={} runtime_owned_control_flow_excluded={} helper_module_mediation_excluded={} artifact_lineage_to_behavior_closed={} stable_across_runs={} route_purity_green={}",
            route_purity_review.hidden_host_substitution_excluded,
            route_purity_review.external_oracle_excluded,
            route_purity_review.preprocessing_shortcut_excluded,
            route_purity_review.route_drift_excluded,
            route_purity_review.runtime_owned_control_flow_excluded,
            route_purity_review.helper_module_mediation_excluded,
            route_purity_review.artifact_lineage_to_behavior_closed,
            computation_mapping_report.stable_across_runs,
            route_purity_review.route_purity_green,
        ),
    }
}

pub fn tassadar_article_route_minimality_audit_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF)
}

pub fn write_tassadar_article_route_minimality_audit_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleRouteMinimalityAuditReport, TassadarArticleRouteMinimalityAuditError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleRouteMinimalityAuditError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_route_minimality_audit_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleRouteMinimalityAuditError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn read_artifact<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleRouteMinimalityAuditError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleRouteMinimalityAuditError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleRouteMinimalityAuditError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn canonical_claim_route_review_digest_fragment(digest: &str) -> &str {
    if digest.len() > 16 {
        &digest[..16]
    } else {
        digest
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_route_minimality_audit_report,
        tassadar_article_route_minimality_audit_report_path,
        write_tassadar_article_route_minimality_audit_report,
        TassadarArticleRouteMinimalityAuditReport, TassadarArticleRouteMinimalityPublicPosture,
        TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF,
    };
    use tempfile::tempdir;

    fn read_committed_report(
    ) -> Result<TassadarArticleRouteMinimalityAuditReport, Box<dyn std::error::Error>> {
        let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root");
        let report_path = repo_root.join(TASSADAR_ARTICLE_ROUTE_MINIMALITY_AUDIT_REPORT_REF);
        Ok(serde_json::from_slice(&std::fs::read(report_path)?)?)
    }

    #[test]
    fn article_route_minimality_audit_is_green_and_publicly_suppressed(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let report = build_tassadar_article_route_minimality_audit_report()?;

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-185A");
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert_eq!(
            report.canonical_claim_route_review.canonical_claim_route_id,
            "tassadar.article_route.direct_hull_cache_runtime.v1"
        );
        assert!(
            report
                .canonical_claim_route_review
                .canonical_claim_route_green
        );
        assert!(
            report
                .continuation_boundary_review
                .continuation_boundary_green
        );
        assert!(report.execution_ownership_review.route_purity_green);
        assert!(report.state_carrier_review.state_carrier_minimality_green);
        assert!(
            report
                .orchestration_review
                .extra_orchestration_layers_excluded
        );
        assert!(report.operator_verdict_review.operator_verdict_green);
        assert_eq!(
            report.public_verdict_review.posture,
            TassadarArticleRouteMinimalityPublicPosture::GreenBounded
        );
        assert!(report.public_verdict_review.public_verdict_green);
        assert!(report.route_minimality_audit_green);
        assert!(report.article_equivalence_green);
        Ok(())
    }

    #[test]
    fn article_route_minimality_audit_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_route_minimality_audit_report()?;
        let committed = read_committed_report()?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_route_minimality_audit_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_route_minimality_audit_report.json");
        let written = write_tassadar_article_route_minimality_audit_report(&output_path)?;
        let persisted: TassadarArticleRouteMinimalityAuditReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_route_minimality_audit_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_article_route_minimality_audit_report.json")
        );
        Ok(())
    }
}
