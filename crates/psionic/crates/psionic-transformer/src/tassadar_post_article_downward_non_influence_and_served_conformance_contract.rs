use serde::{Deserialize, Serialize};

use crate::TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID;

pub const TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_CONTRACT_ID:
    &str = "tassadar.post_article.downward_non_influence_and_served_conformance.contract.v1";
pub const TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_BOUNDARY_ID:
    &str = "tassadar.post_article.downward_non_influence_and_served_conformance.boundary.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleLowerPlaneTruthKind {
    CanonicalComputationalModel,
    CanonicalMachineIdentityLock,
    ExecutionSemanticsProofTransport,
    ContinuationBoundary,
    FastRouteCarrierBinding,
    EquivalentChoiceBoundary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleLowerPlaneTruthRow {
    pub truth_surface_id: String,
    pub truth_kind: TassadarPostArticleLowerPlaneTruthKind,
    pub canonical_machine_binding_required: bool,
    pub plugin_or_served_rewrite_forbidden: bool,
    pub broader_claim_inheritance_allowed: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleDownwardNonInfluenceAndServedConformanceContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub boundary_id: String,
    pub supporting_precedent_ids: Vec<String>,
    pub lower_plane_truth_rows: Vec<TassadarPostArticleLowerPlaneTruthRow>,
    pub allowed_narrower_deviation_ids: Vec<String>,
    pub blocked_rewrite_ids: Vec<String>,
    pub fail_closed_condition_ids: Vec<String>,
    pub contract_rule_rows: Vec<TassadarPostArticleDownwardNonInfluenceLawRow>,
    pub invalidation_rule_rows: Vec<TassadarPostArticleDownwardNonInfluenceLawRow>,
    pub next_stability_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_downward_non_influence_and_served_conformance_contract(
) -> TassadarPostArticleDownwardNonInfluenceAndServedConformanceContract {
    let supporting_precedent_ids = vec![
        String::from("canonical_computational_model_statement"),
        String::from("canonical_machine_identity_lock"),
        String::from("execution_semantics_proof_transport"),
        String::from("continuation_non_computationality"),
        String::from("fast_route_legitimacy_and_carrier_binding"),
        String::from("equivalent_choice_neutrality_and_admissibility"),
        String::from("rebased_universality_verdict_split"),
        String::from("served_conformance_envelope"),
    ];
    let lower_plane_truth_rows = vec![
        lower_plane_row(
            "canonical_computational_model_statement",
            TassadarPostArticleLowerPlaneTruthKind::CanonicalComputationalModel,
            "the canonical computational-model statement names the machine, direct route, continuation inheritance, and effect boundary that later plugin or served surfaces may cite but may not rewrite.",
        ),
        lower_plane_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleLowerPlaneTruthKind::CanonicalMachineIdentityLock,
            "the canonical machine lock binds one machine tuple, one route descriptor digest, and one continuation contract that later surfaces may project but may not recompute.",
        ),
        lower_plane_row(
            "execution_semantics_proof_transport",
            TassadarPostArticleLowerPlaneTruthKind::ExecutionSemanticsProofTransport,
            "the proof-transport boundary freezes which execution semantics survive rebinding and blocks later helper or plugin drift from becoming a stronger machine.",
        ),
        lower_plane_row(
            "continuation_non_computationality",
            TassadarPostArticleLowerPlaneTruthKind::ContinuationBoundary,
            "continuation stays transport-only and may not be widened by plugin or served ergonomics into hidden workflow logic or second-machine computation.",
        ),
        lower_plane_row(
            "fast_route_legitimacy_and_carrier_binding",
            TassadarPostArticleLowerPlaneTruthKind::FastRouteCarrierBinding,
            "fast-route carrier binding keeps only the declared direct route inside the canonical machine while other families remain explicitly narrower or out of carrier.",
        ),
        lower_plane_row(
            "equivalent_choice_neutrality_and_admissibility",
            TassadarPostArticleLowerPlaneTruthKind::EquivalentChoiceBoundary,
            "equivalent-choice neutrality keeps admissibility, narrowing, and typed denial explicit so later served posture cannot smuggle hidden steering back into the lower plane.",
        ),
    ];
    let allowed_narrower_deviation_ids = vec![
        String::from("served_posture_may_remain_narrower_than_operator_universality"),
        String::from("served_posture_may_expose_only_direct_article_route"),
        String::from("served_posture_may_remain_cpu_only_with_explicit_suppression_elsewhere"),
    ];
    let blocked_rewrite_ids = vec![
        String::from("plugin_or_served_rewrites_compute_substrate_rules"),
        String::from("plugin_or_served_rewrites_proof_assumptions"),
        String::from("plugin_or_served_rewrites_continuation_semantics"),
        String::from("plugin_or_served_rewrites_carrier_identity"),
        String::from("plugin_or_served_rewrites_equivalent_choice_boundary"),
        String::from("served_posture_escapes_declared_conformance_envelope"),
        String::from("plugin_capability_or_public_universality_laundered_from_served_posture"),
    ];
    let fail_closed_condition_ids = vec![
        String::from("route_drift_or_descriptor_change"),
        String::from("machine_outside_declared_cpu_matrix"),
        String::from("nonselected_fast_route_claimed_as_universal"),
        String::from("resumable_or_public_universality_widening_attempted"),
        String::from("plugin_capability_implication_attempted"),
    ];
    let contract_rule_rows = vec![
        law_row(
            "lower_plane_truth_rows_must_stay_explicit",
            "Every lower-plane truth surface must stay named explicitly and bound to one canonical machine identity tuple before later capability or served layers may cite it.",
        ),
        law_row(
            "plugin_and_served_layers_may_project_but_not_rewrite",
            "Plugin and served layers may project lower-plane truth into receipts or narrower posture, but they may not rewrite compute-substrate rules, proof assumptions, continuation semantics, carrier identity, or equivalent-choice boundaries.",
        ),
        law_row(
            "served_posture_may_only_narrow_inside_declared_envelope",
            "Served posture may remain narrower than operator truth only through the declared conformance envelope and only by the listed narrower-deviation ids.",
        ),
        law_row(
            "served_widening_must_fail_closed",
            "Any attempt to widen served posture beyond the declared route, machine matrix, decode mode, or publication boundary must fail closed through explicit condition ids.",
        ),
        law_row(
            "historical_and_rebased_verdict_splits_must_agree_on_suppression",
            "Historical and rebased verdict-split surfaces must continue to keep served/public universality, plugin capability, and arbitrary software capability blocked.",
        ),
        law_row(
            "anti_drift_closeout_and_closure_bundle_remain_open",
            "This contract closes downward non-influence and served conformance only; anti-drift closeout and the final closure bundle remain explicit later issues.",
        ),
    ];
    let invalidation_rule_rows = vec![
        law_row(
            "plugin_or_served_rewrites_compute_substrate_invalidates_contract",
            "The contract fails if plugin or served layers redefine the computational model, proof-bearing execution boundary, or lower-plane compute-substrate rules.",
        ),
        law_row(
            "plugin_or_served_rewrites_continuation_invalidates_contract",
            "The contract fails if continuation semantics are widened into hidden workflow logic, second-machine behavior, or stronger execution than the lower plane allows.",
        ),
        law_row(
            "plugin_or_served_rewrites_carrier_identity_invalidates_contract",
            "The contract fails if route-carrier identity, route descriptor digest, decode mode, or machine tuple drift beneath later served or plugin posture.",
        ),
        law_row(
            "plugin_or_served_rewrites_equivalent_choice_invalidates_contract",
            "The contract fails if equivalent-choice or admissibility neutrality is reinterpreted through hidden ordering, ranking, or steering in later layers.",
        ),
        law_row(
            "served_posture_outside_declared_envelope_invalidates_contract",
            "The contract fails if served posture escapes the declared conformance envelope instead of staying narrower and fail-closed.",
        ),
        law_row(
            "served_or_plugin_overclaim_invalidates_contract",
            "The contract fails if narrower served posture is laundered into plugin capability, public universality, or arbitrary software capability claims.",
        ),
        law_row(
            "closure_bundle_overread_invalidates_contract",
            "The final closure bundle may not be inferred from downward non-influence and served conformance alone until later anti-drift closeout and bundle publication land explicitly.",
        ),
    ];

    TassadarPostArticleDownwardNonInfluenceAndServedConformanceContract {
        schema_version: 1,
        contract_id: String::from(
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_CONTRACT_ID,
        ),
        machine_identity_id: String::from(
            crate::TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
        ),
        tuple_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID),
        boundary_id: String::from(
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_BOUNDARY_ID,
        ),
        supporting_precedent_ids,
        lower_plane_truth_rows,
        allowed_narrower_deviation_ids,
        blocked_rewrite_ids,
        fail_closed_condition_ids,
        contract_rule_rows,
        invalidation_rule_rows,
        next_stability_issue_id: String::from("TAS-215"),
        claim_boundary: String::from(
            "this transformer-owned contract freezes only downward non-influence and served conformance for the canonical post-article machine. It binds lower-plane truth surfaces, the allowed narrower served deviations, the fail-closed widening conditions, and the blocked rewrite classes explicitly. It does not itself close anti-drift publication, the canonical machine closure bundle, plugin publication, served/public universality, or arbitrary software capability.",
        ),
        summary: String::from(
            "Transformer downward non-influence and served conformance contract freezes 6 lower-plane truth rows, 3 allowed served deviations, 7 blocked rewrite ids, 5 fail-closed conditions, and 7 invalidation laws for one canonical post-article machine.",
        ),
    }
}

fn lower_plane_row(
    truth_surface_id: &str,
    truth_kind: TassadarPostArticleLowerPlaneTruthKind,
    detail: &str,
) -> TassadarPostArticleLowerPlaneTruthRow {
    TassadarPostArticleLowerPlaneTruthRow {
        truth_surface_id: String::from(truth_surface_id),
        truth_kind,
        canonical_machine_binding_required: true,
        plugin_or_served_rewrite_forbidden: true,
        broader_claim_inheritance_allowed: false,
        green: true,
        detail: String::from(detail),
    }
}

fn law_row(
    rule_id: &str,
    detail: &str,
) -> TassadarPostArticleDownwardNonInfluenceLawRow {
    TassadarPostArticleDownwardNonInfluenceLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_downward_non_influence_and_served_conformance_contract,
        TassadarPostArticleLowerPlaneTruthKind,
        TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_BOUNDARY_ID,
        TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_CONTRACT_ID,
    };

    #[test]
    fn downward_non_influence_contract_covers_lower_plane_truth_and_served_envelope() {
        let contract =
            build_tassadar_post_article_downward_non_influence_and_served_conformance_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_CONTRACT_ID
        );
        assert_eq!(
            contract.boundary_id,
            TASSADAR_POST_ARTICLE_DOWNWARD_NON_INFLUENCE_AND_SERVED_CONFORMANCE_BOUNDARY_ID
        );
        assert_eq!(contract.lower_plane_truth_rows.len(), 6);
        assert_eq!(contract.allowed_narrower_deviation_ids.len(), 3);
        assert_eq!(contract.blocked_rewrite_ids.len(), 7);
        assert_eq!(contract.fail_closed_condition_ids.len(), 5);
        assert!(contract.lower_plane_truth_rows.iter().any(|row| {
            row.truth_surface_id == "continuation_non_computationality"
                && row.truth_kind == TassadarPostArticleLowerPlaneTruthKind::ContinuationBoundary
                && row.plugin_or_served_rewrite_forbidden
        }));
        assert!(contract
            .blocked_rewrite_ids
            .contains(&String::from("served_posture_escapes_declared_conformance_envelope")));
        assert_eq!(contract.next_stability_issue_id, "TAS-215");
    }
}
