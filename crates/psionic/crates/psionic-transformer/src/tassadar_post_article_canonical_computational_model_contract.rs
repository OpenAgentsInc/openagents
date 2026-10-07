use serde::{Deserialize, Serialize};

use crate::{
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
    TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID: &str =
    "tassadar.post_article_universality_bridge.machine_identity.v1";
pub const TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_CONTRACT_ID: &str =
    "tassadar.post_article.canonical_computational_model.contract.v1";
pub const TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_ID: &str =
    "tassadar.post_article.canonical_computational_model.statement.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub computational_model_statement_id: String,
    pub model_rule_rows: Vec<TassadarPostArticleCanonicalComputationalModelLawRow>,
    pub invalidation_rule_rows: Vec<TassadarPostArticleCanonicalComputationalModelLawRow>,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_canonical_computational_model_contract(
) -> TassadarPostArticleCanonicalComputationalModelContract {
    let model_rule_rows = vec![
        law_row(
            "direct_compute_route_must_name_the_owned_transformer_carrier",
            "the published computational model must name the owned direct article-equivalent Transformer route instead of inheriting compute identity from adjacent audits.",
        ),
        law_row(
            "continuation_semantics_must_arrive_only_through_tcm_v1",
            "resumable continuation semantics must attach only through the declared TCM.v1 substrate model and runtime contract instead of through ambient host behavior.",
        ),
        law_row(
            "declared_effect_profiles_bound_compute_and_plugin_layers",
            "declared effect profiles and refusals bound both the rebased compute story and any later plugin overlay instead of being implied by convenience helpers.",
        ),
        law_row(
            "plugin_layer_stays_above_the_machine_not_inside_the_substrate",
            "the plugin layer may sit on top of the same canonical machine identity, but it may not be collapsed into the machine's compute substrate or continuation carrier.",
        ),
        law_row(
            "publication_and_served_posture_stay_out_of_model",
            "plugin publication, served/public universality, and arbitrary software capability remain out of the computational model unless later issues explicitly turn them green.",
        ),
    ];
    let invalidation_rule_rows = vec![
        law_row(
            "canonical_route_or_weight_drift_invalidates_model_statement",
            "a changed canonical model, weight lineage, route id, or route descriptor digest invalidates the statement instead of silently preserving identity.",
        ),
        law_row(
            "continuation_contract_drift_invalidates_model_statement",
            "a changed continuation contract or substrate digest invalidates the statement instead of being inherited as ambient runtime truth.",
        ),
        law_row(
            "effect_boundary_widening_invalidates_model_statement",
            "widened effects, undeclared imports, or implicit publication invalidate the statement instead of becoming part of the model by implication.",
        ),
        law_row(
            "plugin_overlay_recomposition_invalidates_model_statement",
            "treating plugin capability, controller logic, or publication posture as part of the base substrate invalidates the statement instead of enriching it.",
        ),
        law_row(
            "proof_transport_or_terminal_closure_overread_invalidates_model_statement",
            "execution-semantics proof transport and the final closure bundle may not be inferred from this statement until later issues close them explicitly.",
        ),
    ];

    TassadarPostArticleCanonicalComputationalModelContract {
        schema_version: 1,
        contract_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_CONTRACT_ID),
        machine_identity_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID),
        tuple_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID),
        carrier_class_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID),
        computational_model_statement_id: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_ID,
        ),
        model_rule_rows,
        invalidation_rule_rows,
        claim_boundary: String::from(
            "this transformer-owned contract names the canonical computational-model surface only. It does not itself prove execution-semantics transport, publish the final closure bundle, or widen plugin/public/served capability posture.",
        ),
        summary: String::from(
            "Transformer canonical computational-model contract freezes 5 model rules and 5 invalidation rules for one post-article machine identity.",
        ),
    }
}

fn law_row(rule_id: &str, detail: &str) -> TassadarPostArticleCanonicalComputationalModelLawRow {
    TassadarPostArticleCanonicalComputationalModelLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_computational_model_contract,
        TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_CONTRACT_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_ID,
    };
    use crate::{
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
    };

    #[test]
    fn canonical_computational_model_contract_covers_declared_rules() {
        let contract = build_tassadar_post_article_canonical_computational_model_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_CONTRACT_ID
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
            contract.computational_model_statement_id,
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_ID
        );
        assert_eq!(contract.model_rule_rows.len(), 5);
        assert_eq!(contract.invalidation_rule_rows.len(), 5);
        assert!(contract
            .model_rule_rows
            .iter()
            .chain(contract.invalidation_rule_rows.iter())
            .all(|row| row.green));
    }
}
