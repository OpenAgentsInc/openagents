use serde::{Deserialize, Serialize};

use crate::TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID;

pub const TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_ID:
    &str = "tassadar.post_article.equivalent_choice_neutrality_and_admissibility.contract.v1";
pub const TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_BOUNDARY_ID:
    &str = "tassadar.post_article.equivalent_choice_neutrality_and_admissibility.boundary.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleEquivalentChoiceClassKind {
    ClosedWorldNeutralPair,
    SingletonExactAdmissibility,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceNeutralityClassRow {
    pub equivalent_choice_class_id: String,
    pub class_kind: TassadarPostArticleEquivalentChoiceClassKind,
    pub bounded_candidate_count: u32,
    pub neutral_choice_required: bool,
    pub receipt_visible_justification_required: bool,
    pub route_and_mount_binding_required: bool,
    pub hidden_ordering_allowed: bool,
    pub latency_or_cost_discriminator_allowed: bool,
    pub soft_failure_discriminator_allowed: bool,
    pub typed_outcome_required_when_not_admitted: bool,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceNeutralityLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub boundary_id: String,
    pub supporting_precedent_ids: Vec<String>,
    pub blocked_hidden_discriminator_ids: Vec<String>,
    pub equivalent_choice_class_rows: Vec<TassadarPostArticleEquivalentChoiceNeutralityClassRow>,
    pub contract_rule_rows: Vec<TassadarPostArticleEquivalentChoiceNeutralityLawRow>,
    pub invalidation_rule_rows: Vec<TassadarPostArticleEquivalentChoiceNeutralityLawRow>,
    pub next_stability_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract(
) -> TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContract {
    let supporting_precedent_ids = vec![
        String::from("plugin_world_mount_envelope_compiler_and_admissibility"),
        String::from("plugin_runtime_api_and_engine_abstraction"),
        String::from("weighted_plugin_controller_trace_and_refusal_aware_model_loop"),
        String::from("control_plane_decision_provenance_proof"),
        String::from("fast_route_legitimacy_and_carrier_binding"),
        String::from("universality_bridge_contract"),
    ];
    let blocked_hidden_discriminator_ids = vec![
        String::from("hidden_ordering"),
        String::from("hidden_ranking"),
        String::from("latency_steering"),
        String::from("cost_steering"),
        String::from("queue_pressure_steering"),
        String::from("scheduler_order_steering"),
        String::from("cache_hit_steering"),
        String::from("soft_failure_steering"),
        String::from("candidate_precomputation"),
        String::from("hidden_topk_filtering"),
    ];
    let equivalent_choice_class_rows = vec![
        class_row(
            "choice.search_core_pair.closed_world_neutral.v1",
            TassadarPostArticleEquivalentChoiceClassKind::ClosedWorldNeutralPair,
            2,
            true,
            "two bounded search-core candidates remain equivalent only under receipt-visible neutral choice with no hidden ordering, latency, cost, or soft-failure steering.",
        ),
        class_row(
            "choice.validator_search.singleton.v1",
            TassadarPostArticleEquivalentChoiceClassKind::SingletonExactAdmissibility,
            1,
            false,
            "validator-search admissibility remains singleton and exact, so version skew may deny admission but may not become hidden ranking inside an equivalence class.",
        ),
        class_row(
            "choice.strict_no_imports.singleton.v1",
            TassadarPostArticleEquivalentChoiceClassKind::SingletonExactAdmissibility,
            1,
            false,
            "strict no-import admissibility remains singleton and exact, so import posture mismatch is typed denial rather than soft steering.",
        ),
        class_row(
            "choice.missing_dependency.singleton.v1",
            TassadarPostArticleEquivalentChoiceClassKind::SingletonExactAdmissibility,
            1,
            false,
            "missing-dependency admissibility remains singleton and exact, so quarantine stays typed and does not widen or reorder the admissible set.",
        ),
        class_row(
            "choice.public_broad_family.singleton.v1",
            TassadarPostArticleEquivalentChoiceClassKind::SingletonExactAdmissibility,
            1,
            false,
            "served/public broad-family admissibility remains singleton and explicit while publication posture stays suppressed.",
        ),
    ];
    let contract_rule_rows = vec![
        law_row(
            "equivalent_choice_classes_must_be_explicit",
            "Every admissible equivalent-choice class must be named explicitly and remain bound to one canonical machine identity tuple.",
        ),
        law_row(
            "receipt_visible_narrowing_required",
            "Narrowing, filtering, or ordering inside an admissible set must remain receipt-visible and justified by explicit route, mount, version, trust, or publication posture.",
        ),
        law_row(
            "hidden_ordering_and_ranking_blocked",
            "Ordering, ranking, precomputation, and top-k filtering may not become hidden control channels inside an equivalent-choice class.",
        ),
        law_row(
            "latency_and_cost_signals_stay_hidden_from_choice",
            "Latency, cost, queue pressure, scheduler order, and cache-hit signals stay hidden or fixed and may not distinguish admissible equivalent choices.",
        ),
        law_row(
            "soft_failure_effects_must_stay_typed",
            "Soft-failure effects may only appear as typed denied, suppressed, or quarantined outcomes; they may not silently steer one admissible equivalent choice over another.",
        ),
        law_row(
            "later_stability_issues_remain_open",
            "Downward non-influence, served conformance, anti-drift closeout, and the final closure bundle stay open after this contract.",
        ),
    ];
    let invalidation_rule_rows = vec![
        law_row(
            "hidden_ordering_inside_equivalent_class_invalidates_contract",
            "The contract fails if hidden ordering or ranking distinguishes members of an equivalent-choice class.",
        ),
        law_row(
            "latency_or_cost_steering_inside_equivalent_class_invalidates_contract",
            "The contract fails if latency, cost, queue pressure, scheduler order, or cache-hit signals steer one equivalent admissible choice over another.",
        ),
        law_row(
            "soft_failure_steering_invalidates_contract",
            "The contract fails if soft-failure behavior is used to prefer one equivalent admissible choice without a typed receipt-visible outcome.",
        ),
        law_row(
            "unreceipted_narrowing_invalidates_contract",
            "The contract fails if admissibility narrowing occurs without an explicit receipt-visible justification.",
        ),
        law_row(
            "route_or_mount_rebinding_without_contract_invalidates_contract",
            "The contract fails if route or mount rebinding changes the admissible set without explicit canonical machine binding.",
        ),
        law_row(
            "served_or_plugin_overread_invalidates_contract",
            "The contract fails if plugin or served wording overreads admissibility neutrality into broader universality or publication claims.",
        ),
        law_row(
            "closure_bundle_overread_invalidates_contract",
            "The final closure bundle may not be inferred from equivalent-choice neutrality and admissibility alone until later issues land explicitly.",
        ),
    ];

    TassadarPostArticleEquivalentChoiceNeutralityAndAdmissibilityContract {
        schema_version: 1,
        contract_id: String::from(
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_ID,
        ),
        machine_identity_id: String::from(
            crate::TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
        ),
        tuple_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID),
        boundary_id: String::from(
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_BOUNDARY_ID,
        ),
        supporting_precedent_ids,
        blocked_hidden_discriminator_ids,
        equivalent_choice_class_rows,
        contract_rule_rows,
        invalidation_rule_rows,
        next_stability_issue_id: String::from("TAS-215"),
        claim_boundary: String::from(
            "this transformer-owned contract freezes only equivalent-choice neutrality and admissibility on the canonical post-article machine. It makes equivalent-choice classes, receipt-visible narrowing, typed denied or suppressed outcomes, and blocked hidden discriminators explicit. It does not itself close downward non-influence, served conformance, anti-drift closeout, or the final closure bundle.",
        ),
        summary: String::from(
            "Transformer equivalent-choice neutrality and admissibility contract freezes 5 admissibility classes, 6 contract rules, and 7 invalidation rules for one canonical post-article machine.",
        ),
    }
}

fn class_row(
    equivalent_choice_class_id: &str,
    class_kind: TassadarPostArticleEquivalentChoiceClassKind,
    bounded_candidate_count: u32,
    neutral_choice_required: bool,
    detail: &str,
) -> TassadarPostArticleEquivalentChoiceNeutralityClassRow {
    TassadarPostArticleEquivalentChoiceNeutralityClassRow {
        equivalent_choice_class_id: String::from(equivalent_choice_class_id),
        class_kind,
        bounded_candidate_count,
        neutral_choice_required,
        receipt_visible_justification_required: true,
        route_and_mount_binding_required: true,
        hidden_ordering_allowed: false,
        latency_or_cost_discriminator_allowed: false,
        soft_failure_discriminator_allowed: false,
        typed_outcome_required_when_not_admitted: true,
        green: true,
        detail: String::from(detail),
    }
}

fn law_row(
    rule_id: &str,
    detail: &str,
) -> TassadarPostArticleEquivalentChoiceNeutralityLawRow {
    TassadarPostArticleEquivalentChoiceNeutralityLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract,
        TassadarPostArticleEquivalentChoiceClassKind,
        TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_BOUNDARY_ID,
        TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_ID,
    };

    #[test]
    fn equivalent_choice_neutrality_contract_covers_declared_choice_classes() {
        let contract =
            build_tassadar_post_article_equivalent_choice_neutrality_and_admissibility_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_CONTRACT_ID
        );
        assert_eq!(
            contract.boundary_id,
            TASSADAR_POST_ARTICLE_EQUIVALENT_CHOICE_NEUTRALITY_AND_ADMISSIBILITY_BOUNDARY_ID
        );
        assert_eq!(contract.equivalent_choice_class_rows.len(), 5);
        assert!(contract.equivalent_choice_class_rows.iter().any(|row| {
            row.equivalent_choice_class_id == "choice.search_core_pair.closed_world_neutral.v1"
                && row.class_kind
                    == TassadarPostArticleEquivalentChoiceClassKind::ClosedWorldNeutralPair
                && row.bounded_candidate_count == 2
                && row.neutral_choice_required
        }));
        assert!(contract.equivalent_choice_class_rows.iter().all(|row| {
            row.receipt_visible_justification_required
                && row.route_and_mount_binding_required
                && !row.hidden_ordering_allowed
                && !row.latency_or_cost_discriminator_allowed
                && !row.soft_failure_discriminator_allowed
                && row.typed_outcome_required_when_not_admitted
                && row.green
        }));
        assert_eq!(contract.contract_rule_rows.len(), 6);
        assert_eq!(contract.invalidation_rule_rows.len(), 7);
        assert_eq!(contract.next_stability_issue_id, "TAS-215");
    }
}
