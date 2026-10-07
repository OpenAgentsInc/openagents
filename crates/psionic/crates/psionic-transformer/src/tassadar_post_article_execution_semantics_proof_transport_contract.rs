use serde::{Deserialize, Serialize};

use crate::{
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
};

pub const TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_CONTRACT_ID: &str =
    "tassadar.post_article.execution_semantics_proof_transport.contract.v1";
pub const TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_BOUNDARY_ID: &str =
    "tassadar.post_article_universal_machine_proof.proof_transport_boundary.v1";
pub const TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PLUGIN_RUNTIME_PROJECTION_ID: &str =
    "tassadar.post_article.execution_semantics_proof_transport.plugin_runtime_api_projection.v1";
pub const TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PLUGIN_CONFORMANCE_PROJECTION_ID: &str =
    "tassadar.post_article.execution_semantics_proof_transport.plugin_conformance_projection.v1";
pub const TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_WEIGHTED_CONTROLLER_PROJECTION_ID: &str =
    "tassadar.post_article.execution_semantics_proof_transport.weighted_controller_projection.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsProofTransportLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleExecutionSemanticsProofTransportContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub boundary_id: String,
    pub preserved_transition_class_ids: Vec<String>,
    pub admitted_variance_ids: Vec<String>,
    pub blocked_drift_ids: Vec<String>,
    pub plugin_projection_surface_ids: Vec<String>,
    pub transport_rule_rows: Vec<TassadarPostArticleExecutionSemanticsProofTransportLawRow>,
    pub invalidation_rule_rows: Vec<TassadarPostArticleExecutionSemanticsProofTransportLawRow>,
    pub next_stability_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_execution_semantics_proof_transport_contract(
) -> TassadarPostArticleExecutionSemanticsProofTransportContract {
    let preserved_transition_class_ids = vec![
        String::from("bounded_small_step_control_updates"),
        String::from("declared_memory_state_updates"),
        String::from("declared_continuation_resume_equivalence"),
        String::from("declared_effect_boundary_only"),
    ];
    let admitted_variance_ids = vec![
        String::from("canonical_machine_identity_binding"),
        String::from("canonical_model_and_weight_identity_binding"),
        String::from("canonical_route_identity_binding"),
        String::from("carrier_split_publication_without_claim_collapse"),
    ];
    let blocked_drift_ids = vec![
        String::from("helper_substitution"),
        String::from("route_family_drift"),
        String::from("undeclared_cache_owned_control"),
        String::from("undeclared_batching_semantics"),
        String::from("semantic_drift_outside_declared_proof_boundary"),
    ];
    let plugin_projection_surface_ids = vec![
        String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PLUGIN_RUNTIME_PROJECTION_ID),
        String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PLUGIN_CONFORMANCE_PROJECTION_ID),
        String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_WEIGHTED_CONTROLLER_PROJECTION_ID),
    ];
    let transport_rule_rows = vec![
        law_row(
            "proof_transport_boundary_must_match_rebound_small_step_semantics",
            "the proof-transport audit must reuse the same preserved transition classes, admitted variance, and blocked drift already carried by the post-article proof-rebinding surface instead of inventing a looser boundary.",
        ),
        law_row(
            "plugin_runtime_api_must_bind_same_machine_and_continuation_contract",
            "plugin runtime API and engine abstraction may project the canonical machine only if machine identity, route, weight lineage, continuation contract, and computational-model statement stay identical.",
        ),
        law_row(
            "plugin_conformance_harness_must_stay_static_receipt_bound",
            "plugin conformance and benchmark evidence may project the proof boundary only while static host-scripted harnessing, receipt integrity, and explicit envelope compatibility stay frozen.",
        ),
        law_row(
            "weighted_controller_trace_must_keep_host_non_planner",
            "weighted controller traces may project the same machine only while selection, sequencing, refusal handling, and stop decisions remain model-owned and the host stays execution-only.",
        ),
        law_row(
            "proof_transport_audit_must_hand_off_to_later_stability_issues",
            "closing proof transport here does not close fast-route legitimacy, downward non-influence, anti-drift stability, or the final closure bundle.",
        ),
    ];
    let invalidation_rule_rows = vec![
        law_row(
            "helper_substitution_invalidates_proof_transport",
            "helper substitution invalidates proof transport instead of being treated as a harmless implementation detail.",
        ),
        law_row(
            "route_family_drift_invalidates_proof_transport",
            "route-family drift invalidates proof transport instead of being merged into the same proof-bearing machine.",
        ),
        law_row(
            "cache_or_batching_control_drift_invalidates_proof_transport",
            "cache-owned or batching-owned control drift invalidates proof transport instead of being hidden inside runtime scheduling behavior.",
        ),
        law_row(
            "continuation_contract_recomposition_invalidates_proof_transport",
            "changing or recomposing the continuation contract invalidates proof transport instead of being inherited as ambient resumability.",
        ),
        law_row(
            "plugin_surface_machine_mismatch_invalidates_proof_transport",
            "plugin-facing runtime, conformance, or controller surfaces invalidate proof transport if they project a different machine identity, route, weight lineage, or computational-model statement.",
        ),
        law_row(
            "plugin_surface_overclaim_invalidates_proof_transport",
            "plugin-facing wording invalidates proof transport if it claims a stronger machine, stronger proof class, or broader publication posture than the transport boundary actually binds.",
        ),
        law_row(
            "closure_bundle_overread_invalidates_proof_transport",
            "the final closure bundle may not be inferred from the proof-transport audit until later stability issues and TAS-215 close it explicitly.",
        ),
    ];

    TassadarPostArticleExecutionSemanticsProofTransportContract {
        schema_version: 1,
        contract_id: String::from(
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_CONTRACT_ID,
        ),
        machine_identity_id: String::from(
            crate::TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
        ),
        tuple_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID),
        carrier_class_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID),
        boundary_id: String::from(TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_BOUNDARY_ID),
        preserved_transition_class_ids,
        admitted_variance_ids,
        blocked_drift_ids,
        plugin_projection_surface_ids,
        transport_rule_rows,
        invalidation_rule_rows,
        next_stability_issue_id: String::from("TAS-215"),
        claim_boundary: String::from(
            "this transformer-owned contract names the execution-semantics proof-transport boundary only. It does not itself prove fast-route legitimacy, served/public conformance, anti-drift closeout, or the final closure bundle.",
        ),
        summary: String::from(
            "Transformer execution-semantics proof-transport contract freezes 5 transport rules and 7 invalidation rules for one canonical post-article machine boundary.",
        ),
    }
}

fn law_row(
    rule_id: &str,
    detail: &str,
) -> TassadarPostArticleExecutionSemanticsProofTransportLawRow {
    TassadarPostArticleExecutionSemanticsProofTransportLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_execution_semantics_proof_transport_contract,
        TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PLUGIN_CONFORMANCE_PROJECTION_ID,
        TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PLUGIN_RUNTIME_PROJECTION_ID,
        TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_BOUNDARY_ID,
        TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_CONTRACT_ID,
        TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_WEIGHTED_CONTROLLER_PROJECTION_ID,
    };
    use crate::{
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
    };

    #[test]
    fn execution_semantics_proof_transport_contract_covers_declared_rules() {
        let contract = build_tassadar_post_article_execution_semantics_proof_transport_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_CONTRACT_ID
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
            TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PROOF_TRANSPORT_BOUNDARY_ID
        );
        assert_eq!(contract.preserved_transition_class_ids.len(), 4);
        assert_eq!(contract.admitted_variance_ids.len(), 4);
        assert_eq!(contract.blocked_drift_ids.len(), 5);
        assert_eq!(
            contract.plugin_projection_surface_ids,
            vec![
                String::from(
                    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PLUGIN_RUNTIME_PROJECTION_ID
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_PLUGIN_CONFORMANCE_PROJECTION_ID
                ),
                String::from(
                    TASSADAR_POST_ARTICLE_EXECUTION_SEMANTICS_WEIGHTED_CONTROLLER_PROJECTION_ID
                ),
            ]
        );
        assert_eq!(contract.transport_rule_rows.len(), 5);
        assert_eq!(contract.invalidation_rule_rows.len(), 7);
        assert_eq!(contract.next_stability_issue_id, "TAS-215");
        assert!(contract
            .transport_rule_rows
            .iter()
            .chain(contract.invalidation_rule_rows.iter())
            .all(|row| row.green));
    }
}
