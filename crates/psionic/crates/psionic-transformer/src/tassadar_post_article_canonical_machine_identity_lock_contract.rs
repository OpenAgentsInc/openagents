use serde::{Deserialize, Serialize};

pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_CONTRACT_ID: &str =
    "tassadar.post_article.canonical_machine_identity_lock.contract.v1";
pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID: &str =
    "tassadar.post_article.canonical_machine_identity_lock.tuple.v1";
pub const TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID: &str =
    "tassadar.post_article.canonical_machine.closure_bundle_bound_rebased_route_identity.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineIdentityLockLawRow {
    pub rule_id: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineIdentityLockContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub carrier_class_statement: String,
    pub identity_rule_rows: Vec<TassadarPostArticleCanonicalMachineIdentityLockLawRow>,
    pub invalidation_rule_rows: Vec<TassadarPostArticleCanonicalMachineIdentityLockLawRow>,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_canonical_machine_identity_lock_contract(
) -> TassadarPostArticleCanonicalMachineIdentityLockContract {
    let identity_rule_rows = vec![
        law_row(
            "one_globally_named_machine_tuple_required",
            "all post-article proofs, witnesses, benchmarks, routes, receipts, controller traces, and closeouts must bind to one globally named machine tuple instead of inheriting identity implicitly.",
        ),
        law_row(
            "tuple_fields_must_name_model_weight_route_and_continuation",
            "the canonical tuple must name the model id, weight digest, route digest, continuation contract, and carrier class instead of relying on prose-only identity claims.",
        ),
        law_row(
            "legacy_partial_artifacts_must_be_rebound_explicitly",
            "older partial-tuple artifacts may stay as historical reports only if one later machine-readable lock explicitly rebinds them to the canonical tuple.",
        ),
        law_row(
            "plugin_receipts_and_controller_traces_must_inherit_the_tuple",
            "plugin-facing receipts, controller traces, and bounded capability closeouts must inherit the same canonical tuple instead of floating on adjacent runtime or sandbox observations.",
        ),
        law_row(
            "closure_bundle_separation_must_remain_explicit",
            "freezing the canonical machine identity does not by itself publish the final closure bundle, served/public universality, or arbitrary software capability.",
        ),
    ];
    let invalidation_rule_rows = vec![
        law_row(
            "model_identity_drift_invalidates_lock",
            "a changed model id or weight lineage invalidates the lock instead of being silently treated as the same machine.",
        ),
        law_row(
            "weight_digest_drift_invalidates_lock",
            "a changed canonical weight bundle digest invalidates the lock instead of being treated as a harmless refresh.",
        ),
        law_row(
            "route_digest_drift_invalidates_lock",
            "a changed canonical route or route descriptor digest invalidates the lock instead of being merged into the same truth carrier.",
        ),
        law_row(
            "continuation_contract_drift_invalidates_lock",
            "a changed continuation contract invalidates the lock instead of being inherited as ambient runtime behavior.",
        ),
        law_row(
            "dual_truth_carrier_recomposition_invalidates_lock",
            "mixing direct, resumable, and capability evidence without one explicit lock invalidates the claim instead of creating a larger implied machine.",
        ),
        law_row(
            "publication_or_served_overread_invalidates_lock",
            "plugin publication, served/public universality, or arbitrary software capability may not be inferred from the lock unless separately green elsewhere.",
        ),
    ];

    TassadarPostArticleCanonicalMachineIdentityLockContract {
        schema_version: 1,
        contract_id: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_CONTRACT_ID,
        ),
        tuple_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID),
        carrier_class_id: String::from(TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID),
        carrier_class_statement: String::from(
            "the canonical machine carrier class is one closure-bundle-bound rebased route identity above the owned transformer stack; direct-route, resumable, control, and capability artifacts are projections of that same machine rather than separate underlying machines.",
        ),
        identity_rule_rows,
        invalidation_rule_rows,
        claim_boundary: String::from(
            "this transformer-owned contract names the canonical machine-identity lock surface only. It does not itself publish the computational-model statement, the final closure bundle, plugin publication, served/public universality, or arbitrary software capability.",
        ),
        summary: String::from(
            "Transformer canonical-machine lock contract freezes 5 identity rules and 6 invalidation rules for one globally named post-article machine tuple.",
        ),
    }
}

fn law_row(rule_id: &str, detail: &str) -> TassadarPostArticleCanonicalMachineIdentityLockLawRow {
    TassadarPostArticleCanonicalMachineIdentityLockLawRow {
        rule_id: String::from(rule_id),
        green: true,
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_machine_identity_lock_contract,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_CONTRACT_ID,
        TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID,
    };

    #[test]
    fn canonical_machine_identity_lock_contract_covers_declared_rules() {
        let contract = build_tassadar_post_article_canonical_machine_identity_lock_contract();

        assert_eq!(
            contract.contract_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_IDENTITY_LOCK_CONTRACT_ID
        );
        assert_eq!(
            contract.tuple_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_TUPLE_ID
        );
        assert_eq!(
            contract.carrier_class_id,
            TASSADAR_POST_ARTICLE_CANONICAL_MACHINE_CARRIER_CLASS_ID
        );
        assert_eq!(contract.identity_rule_rows.len(), 5);
        assert_eq!(contract.invalidation_rule_rows.len(), 6);
        assert!(contract
            .identity_rule_rows
            .iter()
            .chain(contract.invalidation_rule_rows.iter())
            .all(|row| row.green));
    }
}
