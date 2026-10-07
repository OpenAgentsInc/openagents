use serde::{Deserialize, Serialize};

pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID: &str =
    "tassadar.weighted_plugin.controller_trace_contract.v1";
pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID: &str =
    "tassadar.weighted_plugin.control_trace_profile.v1";
pub const TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID: &str =
    "tassadar.weighted_plugin.controller_determinism_profile.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleWeightedPluginControllerTraceContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub control_trace_profile_id: String,
    pub determinism_profile_id: String,
    pub ownership_rule_rows: Vec<TassadarPostArticleWeightedPluginControllerTraceLawRow>,
    pub determinism_rule_rows: Vec<TassadarPostArticleWeightedPluginControllerTraceLawRow>,
    pub host_boundary_rule_rows: Vec<TassadarPostArticleWeightedPluginControllerTraceLawRow>,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_weighted_plugin_controller_trace_contract(
) -> TassadarPostArticleWeightedPluginControllerTraceContract {
    let ownership_rule_rows = vec![
        law_row(
            "model_selects_plugin",
            "plugin choice must remain weight-owned instead of host-ranked or host-preselected.",
        ),
        law_row(
            "model_selects_export",
            "export choice must remain weight-owned instead of falling back to host heuristics or default exports.",
        ),
        law_row(
            "model_constructs_packet_arguments",
            "packet arguments must be derived from model outputs under the canonical packet ABI instead of host-authored argument synthesis.",
        ),
        law_row(
            "model_owns_multi_step_sequencing",
            "multi-step plugin sequencing must remain weight-owned instead of becoming host workflow choreography.",
        ),
        law_row(
            "model_decides_retry_or_refusal",
            "retry versus refusal posture must remain explicit in the model loop instead of collapsing into hidden runtime retry policy.",
        ),
        law_row(
            "model_decides_completion_stop",
            "completion and stop conditions must remain weight-owned instead of being inferred from host convenience or queue policy.",
        ),
    ];
    let determinism_rule_rows = vec![
        law_row(
            "selected_determinism_class_declared",
            "the controller trace must declare one determinism class instead of relying on ambient runtime behavior.",
        ),
        law_row(
            "sampling_policy_declared",
            "sampling policy must remain explicit and challengeable rather than implicit in the runtime.",
        ),
        law_row(
            "temperature_and_randomness_controls_declared",
            "temperature and randomness controls must remain explicit even when the selected route is deterministic.",
        ),
        law_row(
            "external_signal_boundary_explicit",
            "latency, cost, scheduling, cache, and helper-selection signals must remain outside the model-visible controller surface.",
        ),
    ];
    let host_boundary_rule_rows = vec![
        law_row(
            "host_validates_and_executes_but_does_not_plan",
            "the host may validate manifests and execute declared calls, but it may not become the planner.",
        ),
        law_row(
            "hidden_host_side_sequencing_forbidden",
            "hidden host-side sequencing is forbidden instead of being smuggled in as workflow glue.",
        ),
        law_row(
            "host_auto_retry_forbidden",
            "host auto-retry is forbidden and must remain a typed model-visible choice instead.",
        ),
        law_row(
            "fallback_export_selection_forbidden",
            "fallback export selection is forbidden when the declared export refuses or is absent.",
        ),
        law_row(
            "heuristic_plugin_ranking_forbidden",
            "heuristic plugin ranking is forbidden unless the ranking itself is explicit model-visible state.",
        ),
        law_row(
            "schema_auto_repair_forbidden",
            "schema auto-repair is forbidden instead of mutating packet or result meaning after the model emits a call.",
        ),
        law_row(
            "cached_result_substitution_forbidden",
            "cached result substitution is forbidden unless the cache path is explicit in the declared replay contract.",
        ),
        law_row(
            "candidate_precomputation_and_hidden_topk_forbidden",
            "candidate precomputation and hidden top-k filtering are forbidden because they make the host an undeclared planner.",
        ),
        law_row(
            "helper_substitution_forbidden",
            "adversarial helper substitution is forbidden instead of treating nearby helper surfaces as equivalent controller outcomes.",
        ),
        law_row(
            "runtime_learning_or_policy_drift_forbidden",
            "runtime learning or policy drift is forbidden because the controller must stay weight-owned and replay-stable.",
        ),
    ];

    TassadarPostArticleWeightedPluginControllerTraceContract {
        schema_version: 1,
        contract_id: String::from(TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID),
        control_trace_profile_id: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID,
        ),
        determinism_profile_id: String::from(
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID,
        ),
        ownership_rule_rows,
        determinism_rule_rows,
        host_boundary_rule_rows,
        claim_boundary: String::from(
            "this transformer-owned contract freezes the weighted plugin controller trace above the canonical owned route. It keeps plugin selection, export selection, packet-argument construction, sequencing, refusal, retry, and stop conditions weight-owned while making determinism, sampling, randomness, external-signal boundaries, and host-negative planner attacks explicit without yet claiming publication, trust-tier widening, served/public universality, or arbitrary public software execution.",
        ),
        summary: String::from(
            "Transformer weighted-controller contract freezes 6 ownership rules, 4 determinism rules, and 10 host-boundary rules for the post-article plugin controller trace.",
        ),
    }
}

fn law_row(rule_id: &str, detail: &str) -> TassadarPostArticleWeightedPluginControllerTraceLawRow {
    TassadarPostArticleWeightedPluginControllerTraceLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_weighted_plugin_controller_trace_contract,
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID,
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID,
        TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID,
    };

    #[test]
    fn post_article_weighted_plugin_controller_trace_contract_covers_declared_rules() {
        let contract = build_tassadar_post_article_weighted_plugin_controller_trace_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_TRACE_CONTRACT_ID
        );
        assert_eq!(
            contract.control_trace_profile_id,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROL_TRACE_PROFILE_ID
        );
        assert_eq!(
            contract.determinism_profile_id,
            TASSADAR_POST_ARTICLE_WEIGHTED_PLUGIN_CONTROLLER_DETERMINISM_PROFILE_ID
        );
        assert_eq!(contract.ownership_rule_rows.len(), 6);
        assert_eq!(contract.determinism_rule_rows.len(), 4);
        assert_eq!(contract.host_boundary_rule_rows.len(), 10);
        assert!(contract
            .ownership_rule_rows
            .iter()
            .chain(contract.determinism_rule_rows.iter())
            .chain(contract.host_boundary_rule_rows.iter())
            .all(|row| row.green));
    }
}
