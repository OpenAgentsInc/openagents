use serde::{Deserialize, Serialize};

pub const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID: &str =
    "tassadar.weighted_plugin.result_binding_contract.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID: &str =
    "tassadar.weighted_plugin.model_loop_return_profile.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub model_loop_return_profile_id: String,
    pub schema_evolution_rule_rows: Vec<TassadarPostArticlePluginResultBindingLawRow>,
    pub refusal_normalization_rule_rows: Vec<TassadarPostArticlePluginResultBindingLawRow>,
    pub composition_law_rows: Vec<TassadarPostArticlePluginResultBindingLawRow>,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_plugin_result_binding_contract(
) -> TassadarPostArticlePluginResultBindingContract {
    let schema_evolution_rule_rows = vec![
        law_row(
            "exact_schema_identity_or_declared_backward_compatibility",
            "plugin outputs may only re-enter the model loop under exact schema identity or an explicitly declared backward-compatible evolution rule.",
        ),
        law_row(
            "output_digest_binds_next_model_visible_state",
            "every admitted plugin output digest must bind to the next model-visible state digest explicitly instead of relying on adapter-defined reshaping.",
        ),
        law_row(
            "lossy_field_coercion_refused",
            "lossy coercion or field dropping remains a typed refusal rather than a silent reinjection convenience.",
        ),
        law_row(
            "semantic_incompleteness_fails_closed",
            "schema projections that leave task meaning semantically incomplete remain fail-closed instead of adapter-defined partial success.",
        ),
    ];
    let refusal_normalization_rule_rows = vec![
        law_row(
            "typed_failure_classes_preserved",
            "typed refusal and failure classes remain preserved all the way into the model-visible return state.",
        ),
        law_row(
            "retryable_vs_terminal_refusals_explicit",
            "retryable and terminal refusal classes remain explicit instead of collapsing into one generic model-visible error token.",
        ),
        law_row(
            "observational_audits_do_not_rewrite_proof_state",
            "observational result audits remain distinct from proof-carrying result guarantees and may not rewrite the reinjection contract.",
        ),
        law_row(
            "version_skew_refuses_before_reinjection",
            "model-version versus plugin-schema version skew remains a fail-closed refusal before reinjection.",
        ),
    ];
    let composition_law_rows = vec![
        law_row(
            "multi_step_semantic_closure_required",
            "multi-step chaining remains semantically closed or fails closed with typed refusal.",
        ),
        law_row(
            "non_lossy_schema_transition_required",
            "schema transitions across chained plugin outputs remain non-lossy or refuse reinjection.",
        ),
        law_row(
            "ambiguous_composition_blocked",
            "composition across steps that introduces ambiguity remains blocked instead of letting host adapters choose a meaning.",
        ),
        law_row(
            "adapter_defined_return_path_forbidden",
            "the model-loop return path stays contract-defined and replayable rather than adapter-defined per caller or plugin family.",
        ),
    ];

    TassadarPostArticlePluginResultBindingContract {
        schema_version: 1,
        contract_id: String::from(TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID),
        model_loop_return_profile_id: String::from(
            TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
        ),
        schema_evolution_rule_rows,
        refusal_normalization_rule_rows,
        composition_law_rows,
        claim_boundary: String::from(
            "this transformer-owned contract freezes the abstract weighted plugin result-binding surface above the canonical owned route. It keeps schema evolution, refusal normalization, explicit output-to-state digest binding, semantic closure, non-lossy composition, and fail-closed ambiguity posture explicit without yet claiming that weighted plugin sequencing itself is closed.",
        ),
        summary: String::from(
            "Transformer result-binding contract freezes 4 schema-evolution rules, 4 refusal-normalization rules, and 4 composition laws for weighted plugin return-path ownership.",
        ),
    }
}

fn law_row(rule_id: &str, detail: &str) -> TassadarPostArticlePluginResultBindingLawRow {
    TassadarPostArticlePluginResultBindingLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_plugin_result_binding_contract,
        TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID,
    };

    #[test]
    fn post_article_plugin_result_binding_contract_covers_declared_rules() {
        let contract = build_tassadar_post_article_plugin_result_binding_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID
        );
        assert_eq!(
            contract.model_loop_return_profile_id,
            TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID
        );
        assert_eq!(contract.schema_evolution_rule_rows.len(), 4);
        assert_eq!(contract.refusal_normalization_rule_rows.len(), 4);
        assert_eq!(contract.composition_law_rows.len(), 4);
        assert!(contract
            .schema_evolution_rule_rows
            .iter()
            .all(|row| row.green));
        assert!(contract
            .refusal_normalization_rule_rows
            .iter()
            .all(|row| row.green));
        assert!(contract.composition_law_rows.iter().all(|row| row.green));
    }
}
