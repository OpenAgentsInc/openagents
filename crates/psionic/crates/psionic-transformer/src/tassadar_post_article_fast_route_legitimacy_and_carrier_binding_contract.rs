use serde::{Deserialize, Serialize};

use crate::{
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
};

pub const TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_ID: &str =
    "tassadar.post_article.fast_route_legitimacy_and_carrier_binding.contract.v1";
pub const TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_BOUNDARY_ID: &str =
    "tassadar.post_article.fast_route_legitimacy_and_carrier_binding.boundary.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleFastRouteFamilyClass {
    ReferenceLinearBaseline,
    CanonicalHullCache,
    ResumableContinuationFamily,
    ResearchOnlyFastRoute,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleFastRouteCarrierRelation {
    HistoricalProofBaseline,
    CanonicalDirectCarrierBound,
    ContinuationCarrierOnly,
    OutsideCarrier,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteCarrierBindingRow {
    pub route_family_id: String,
    pub route_family_class: TassadarPostArticleFastRouteFamilyClass,
    pub carrier_relation: TassadarPostArticleFastRouteCarrierRelation,
    pub semantics_equivalence_required_for_proof_or_universality: bool,
    pub served_or_plugin_machine_claim_allowed: bool,
    pub route_family_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteLegitimacyLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub boundary_id: String,
    pub semantics_equivalence_evidence_ids: Vec<String>,
    pub blocked_overclaim_ids: Vec<String>,
    pub route_family_rows: Vec<TassadarPostArticleFastRouteCarrierBindingRow>,
    pub contract_rule_rows: Vec<TassadarPostArticleFastRouteLegitimacyLawRow>,
    pub invalidation_rule_rows: Vec<TassadarPostArticleFastRouteLegitimacyLawRow>,
    pub next_stability_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract(
) -> TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContract {
    let semantics_equivalence_evidence_ids = vec![
        String::from("article_fast_route_architecture_selection"),
        String::from("article_fast_route_implementation"),
        String::from("canonical_route_semantic_preservation"),
        String::from("execution_semantics_proof_transport"),
        String::from("canonical_machine_identity_lock"),
    ];
    let blocked_overclaim_ids = vec![
        String::from("unproven_fast_route_inherits_proof"),
        String::from("unproven_fast_route_inherits_universality"),
        String::from("resumable_family_presented_as_direct_machine"),
        String::from("served_or_plugin_wording_recomposes_machine"),
        String::from("research_only_fast_route_promoted_inside_carrier"),
        String::from("hidden_route_descriptor_drift"),
    ];
    let route_family_rows = vec![
        route_family_row(
            "reference_linear",
            TassadarPostArticleFastRouteFamilyClass::ReferenceLinearBaseline,
            TassadarPostArticleFastRouteCarrierRelation::HistoricalProofBaseline,
            true,
            false,
            "ReferenceLinear remains the historical direct-proof baseline and dense semantic anchor. It may justify later carrier inheritance only through explicit semantics-equivalence evidence rather than ambient fast-path inheritance.",
        ),
        route_family_row(
            "hull_cache",
            TassadarPostArticleFastRouteFamilyClass::CanonicalHullCache,
            TassadarPostArticleFastRouteCarrierRelation::CanonicalDirectCarrierBound,
            true,
            true,
            "HullCache may sit inside the canonical direct carrier only when route selection, implementation, semantic-preservation, proof-transport, and canonical-machine binding stay explicit and green.",
        ),
        route_family_row(
            "resumable_continuation_family",
            TassadarPostArticleFastRouteFamilyClass::ResumableContinuationFamily,
            TassadarPostArticleFastRouteCarrierRelation::ContinuationCarrierOnly,
            true,
            false,
            "Resumable continuation remains a carrier extension on the same machine, not a second direct fast route and not the underneath machine for platform wording.",
        ),
        route_family_row(
            "linear_recurrent_runtime",
            TassadarPostArticleFastRouteFamilyClass::ResearchOnlyFastRoute,
            TassadarPostArticleFastRouteCarrierRelation::OutsideCarrier,
            true,
            false,
            "The recurrent runtime stays outside the canonical carrier until it owns a canonical decode family, semantics-equivalence evidence, and explicit machine binding.",
        ),
        route_family_row(
            "hierarchical_hull_research",
            TassadarPostArticleFastRouteFamilyClass::ResearchOnlyFastRoute,
            TassadarPostArticleFastRouteCarrierRelation::OutsideCarrier,
            true,
            false,
            "Hierarchical-hull remains outside the carrier as bounded research evidence rather than a proof-bearing or platform-bearing route.",
        ),
        route_family_row(
            "two_dimensional_head_hard_max_research",
            TassadarPostArticleFastRouteFamilyClass::ResearchOnlyFastRoute,
            TassadarPostArticleFastRouteCarrierRelation::OutsideCarrier,
            true,
            false,
            "The 2D-head hard-max family remains outside the carrier because the repo still exposes it only as a bounded research lane with hull fallback.",
        ),
    ];
    let contract_rule_rows = vec![
        law_row(
            "reference_linear_is_baseline_not_ambient_machine",
            "ReferenceLinear stays an explicit proof baseline and semantic anchor; later fast routes inherit from it only on declared evidence.",
        ),
        law_row(
            "canonical_hull_cache_requires_explicit_equivalence_and_binding",
            "HullCache becomes canonical only while route selection, implementation, semantic-preservation, proof transport, and machine lock remain jointly green.",
        ),
        law_row(
            "resumable_family_stays_continuation_only",
            "Resumable continuation may extend the same machine carrier, but it may not be presented as the direct compute lane or a separate proof-bearing route family.",
        ),
        law_row(
            "research_families_stay_outside_carrier_until_promoted_explicitly",
            "Research-only fast families stay outside the carrier until they own canonical route contracts, semantics-equivalence evidence, and machine binding.",
        ),
        law_row(
            "served_and_plugin_wording_must_name_bound_machine_not_family_alias",
            "Served or plugin wording may name only the closure-bundle-bound canonical machine tuple, not an unbound fast-family alias or research lane.",
        ),
        law_row(
            "later_stability_issues_remain_open",
            "Downward non-influence, served conformance, anti-drift closeout, and final closure-bundle completion stay open after this contract.",
        ),
    ];
    let invalidation_rule_rows = vec![
        law_row(
            "proof_inheritance_without_equivalence_invalidates_contract",
            "The contract fails if proof status is transferred to a fast route without explicit semantics-equivalence evidence.",
        ),
        law_row(
            "universality_inheritance_without_equivalence_invalidates_contract",
            "The contract fails if universality status is transferred to a fast route without explicit carrier binding and proof-transport evidence.",
        ),
        law_row(
            "resumable_family_presented_as_direct_machine_invalidates_contract",
            "The contract fails if resumable continuation is presented as the underlying direct machine rather than a continuation carrier.",
        ),
        law_row(
            "research_fast_route_promoted_inside_carrier_invalidates_contract",
            "The contract fails if a research-only fast family is promoted into production, served, or plugin wording without canonical binding.",
        ),
        law_row(
            "served_or_plugin_wording_on_unbound_route_invalidates_contract",
            "The contract fails if served or plugin surfaces call an unbound fast family the machine underneath the platform.",
        ),
        law_row(
            "route_descriptor_drift_without_explicit_classification_invalidates_contract",
            "The contract fails if reference, canonical fast, and continuation route descriptors drift without an explicit carrier classification that explains the relationship.",
        ),
        law_row(
            "closure_bundle_overread_invalidates_contract",
            "The final closure bundle may not be inferred from fast-route legitimacy alone until later stability issues land explicitly.",
        ),
    ];

    TassadarPostArticleFastRouteLegitimacyAndCarrierBindingContract {
        schema_version: 1,
        contract_id: String::from(
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_ID,
        ),
        machine_identity_id: String::from(
            crate::TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
        ),
        tuple_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID),
        carrier_class_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID),
        boundary_id: String::from(
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_BOUNDARY_ID,
        ),
        semantics_equivalence_evidence_ids,
        blocked_overclaim_ids,
        route_family_rows,
        contract_rule_rows,
        invalidation_rule_rows,
        next_stability_issue_id: String::from("TAS-215"),
        claim_boundary: String::from(
            "this transformer-owned contract freezes only fast-route legitimacy and carrier binding. It classifies which route families are inside the canonical machine carrier, which remain continuation-only, and which stay outside until later promotion. It does not itself close downward non-influence, served conformance, anti-drift closeout, or the final closure bundle.",
        ),
        summary: String::from(
            "Transformer fast-route legitimacy and carrier-binding contract freezes 6 route-family classifications, 6 contract rules, and 7 invalidation rules for one canonical post-article machine.",
        ),
    }
}

fn route_family_row(
    route_family_id: &str,
    route_family_class: TassadarPostArticleFastRouteFamilyClass,
    carrier_relation: TassadarPostArticleFastRouteCarrierRelation,
    semantics_equivalence_required_for_proof_or_universality: bool,
    served_or_plugin_machine_claim_allowed: bool,
    detail: &str,
) -> TassadarPostArticleFastRouteCarrierBindingRow {
    TassadarPostArticleFastRouteCarrierBindingRow {
        route_family_id: String::from(route_family_id),
        route_family_class,
        carrier_relation,
        semantics_equivalence_required_for_proof_or_universality,
        served_or_plugin_machine_claim_allowed,
        route_family_green: true,
        detail: String::from(detail),
    }
}

fn law_row(rule_id: &str, detail: &str) -> TassadarPostArticleFastRouteLegitimacyLawRow {
    TassadarPostArticleFastRouteLegitimacyLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract,
        TassadarPostArticleFastRouteCarrierRelation,
        TassadarPostArticleFastRouteFamilyClass,
        TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_BOUNDARY_ID,
        TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_ID,
    };
    use crate::{
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
    };

    #[test]
    fn fast_route_legitimacy_and_carrier_binding_contract_covers_declared_route_families() {
        let contract =
            build_tassadar_post_article_fast_route_legitimacy_and_carrier_binding_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_CONTRACT_ID
        );
        assert_eq!(
            contract.machine_identity_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID
        );
        assert_eq!(
            contract.tuple_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID
        );
        assert_eq!(
            contract.carrier_class_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID
        );
        assert_eq!(
            contract.boundary_id,
            TASSADAR_POST_ARTICLE_FAST_ROUTE_LEGITIMACY_AND_CARRIER_BINDING_BOUNDARY_ID
        );
        assert_eq!(contract.route_family_rows.len(), 6);
        assert_eq!(contract.contract_rule_rows.len(), 6);
        assert_eq!(contract.invalidation_rule_rows.len(), 7);
        assert_eq!(contract.next_stability_issue_id, "TAS-215");
        assert_eq!(
            contract.route_family_rows[0].route_family_class,
            TassadarPostArticleFastRouteFamilyClass::ReferenceLinearBaseline
        );
        assert_eq!(
            contract.route_family_rows[1].carrier_relation,
            TassadarPostArticleFastRouteCarrierRelation::CanonicalDirectCarrierBound
        );
        assert_eq!(
            contract.route_family_rows[2].carrier_relation,
            TassadarPostArticleFastRouteCarrierRelation::ContinuationCarrierOnly
        );
        assert!(contract
            .route_family_rows
            .iter()
            .all(|row| row.route_family_green));
        assert!(contract.contract_rule_rows.iter().all(|row| row.green));
        assert!(contract
            .invalidation_rule_rows
            .iter()
            .all(|row| row.green));
    }
}
