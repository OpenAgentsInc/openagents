use serde::{Deserialize, Serialize};

const CONTRACT_ID: &str = "tassadar.post_article_anti_drift_stability_closeout.contract.v1";
const MACHINE_IDENTITY_ID: &str = "tassadar.post_article_universality_bridge.machine_identity.v1";
const CANONICAL_ARCHITECTURE_ANCHOR_CRATE: &str = "psionic-transformer";
const CANONICAL_ARCHITECTURE_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleAntiDriftSurfaceClass {
    RuntimeStatement,
    IdentityLock,
    ProofCarrying,
    Audit,
    BoundaryContract,
    CapabilityCloseout,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftRequiredSurfaceRow {
    pub surface_id: String,
    pub surface_class: TassadarPostArticleAntiDriftSurfaceClass,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftInvalidationLaw {
    pub invalidation_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftClaimBlockRow {
    pub claim_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleAntiDriftStabilityCloseoutContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub required_surface_rows: Vec<TassadarPostArticleAntiDriftRequiredSurfaceRow>,
    pub invalidation_laws: Vec<TassadarPostArticleAntiDriftInvalidationLaw>,
    pub stronger_claim_blocks: Vec<TassadarPostArticleAntiDriftClaimBlockRow>,
    pub closure_bundle_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_anti_drift_stability_closeout_contract(
) -> TassadarPostArticleAntiDriftStabilityCloseoutContract {
    let required_surface_rows = vec![
        required_surface_row(
            "canonical_computational_model_statement",
            TassadarPostArticleAntiDriftSurfaceClass::RuntimeStatement,
            "the published computational-model statement must stay explicit so compute identity, continuation inheritance, effect boundaries, and plugin-above-machine posture cannot drift back into adjacent prose.",
        ),
        required_surface_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleAntiDriftSurfaceClass::IdentityLock,
            "the canonical machine lock must keep one tuple-bound machine identity explicit so later reports cannot silently recombine proofs, routes, or receipts onto different effective machines.",
        ),
        required_surface_row(
            "control_plane_decision_provenance_proof",
            TassadarPostArticleAntiDriftSurfaceClass::ProofCarrying,
            "the control-plane provenance proof must keep branch, retry, stop, replay, determinism, failure, time, information-boundary, hidden-state, and observer requirements bound to the same machine identity.",
        ),
        required_surface_row(
            "execution_semantics_proof_transport_audit",
            TassadarPostArticleAntiDriftSurfaceClass::Audit,
            "the proof-transport audit must keep the historical universal-machine proof attached to the canonical post-article route instead of allowing proof drift through route-family substitution.",
        ),
        required_surface_row(
            "continuation_non_computationality_boundary",
            TassadarPostArticleAntiDriftSurfaceClass::BoundaryContract,
            "the continuation boundary must stay explicit so checkpoint, spill, session, process-object, and installed-process surfaces extend the machine without becoming a second machine.",
        ),
        required_surface_row(
            "fast_route_legitimacy_and_carrier_binding",
            TassadarPostArticleAntiDriftSurfaceClass::BoundaryContract,
            "fast-route legitimacy must stay locked so the canonical HullCache carrier remains explicit and unproven fast families stay quarantined outside the machine.",
        ),
        required_surface_row(
            "equivalent_choice_neutrality_and_admissibility",
            TassadarPostArticleAntiDriftSurfaceClass::BoundaryContract,
            "equivalent-choice neutrality must stay explicit so admissible plugin variation remains receipt-visible and cannot reopen hidden ordering, ranking, cost, or latency control.",
        ),
        required_surface_row(
            "downward_non_influence_and_served_conformance",
            TassadarPostArticleAntiDriftSurfaceClass::BoundaryContract,
            "downward non-influence must stay explicit so later plugin or served ergonomics cannot rewrite lower-plane truth or widen the served posture beyond the declared fail-closed envelope.",
        ),
        required_surface_row(
            "rebased_universality_verdict_split",
            TassadarPostArticleAntiDriftSurfaceClass::Audit,
            "the rebased verdict split must stay explicit so theory/operator truth remains green while served/public universality remains suppressed on the canonical route.",
        ),
        required_surface_row(
            "universality_portability_minimality_matrix",
            TassadarPostArticleAntiDriftSurfaceClass::Audit,
            "portability and minimality must stay explicit so machine-class drift, route-carrier drift, and component-boundary drift remain visible instead of collapsing into a single-host anecdote.",
        ),
        required_surface_row(
            "plugin_charter_authority_boundary",
            TassadarPostArticleAntiDriftSurfaceClass::BoundaryContract,
            "the plugin charter must keep state classes, proof-versus-audit distinction, observer posture, host-negative authority, and downward non-influence explicit above the same canonical machine.",
        ),
        required_surface_row(
            "bounded_weighted_plugin_platform_closeout",
            TassadarPostArticleAntiDriftSurfaceClass::CapabilityCloseout,
            "the bounded plugin-platform closeout must keep operator-only capability posture explicit without turning plugin publication, served/public universality, or arbitrary software capability green.",
        ),
    ];

    let invalidation_laws = vec![
        invalidation_law(
            "mixed_carrier_recomposition_rejected",
            "mixed-carrier recomposition stays invalid: no claim may splice proof, route, continuation, or plugin artifacts from different effective machines and still count as one canonical machine.",
        ),
        invalidation_law(
            "route_drift_rejected",
            "route drift stays invalid: the declared canonical route id and descriptor digest must remain stable across proof-bearing and boundary-bearing artifacts.",
        ),
        invalidation_law(
            "determinism_mismatch_rejected",
            "determinism drift stays invalid: control-plane replay posture, determinism class, and equivalent-choice relation must remain explicit and machine-checkable.",
        ),
        invalidation_law(
            "hidden_state_channel_rejected",
            "hidden-state drift stays invalid: hidden state channels, hidden workflow logic, and observer-model mismatch cannot be reintroduced by continuation or plugin ergonomics.",
        ),
        invalidation_law(
            "fast_route_substitution_rejected",
            "undeclared fast-route substitution stays invalid: unproven fast families remain outside the canonical carrier until explicitly promoted.",
        ),
        invalidation_law(
            "downward_influence_rejected",
            "downward influence stays invalid: later plugin, served, or publication layers may not rewrite compute identity, semantics, continuation boundaries, or choice neutrality.",
        ),
        invalidation_law(
            "portability_or_minimality_failure_rejected",
            "portability and minimality drift stay invalid: machine-class posture, route classification, and state-carrier minimality must remain explicit on the canonical route.",
        ),
        invalidation_law(
            "served_or_plugin_overclaim_rejected",
            "served or plugin overclaim stays invalid: bounded operator/internal capability does not imply plugin publication, served/public universality, or arbitrary software capability.",
        ),
    ];

    let stronger_claim_blocks = vec![
        claim_block(
            "terminal_universality_requires_closure_bundle",
            "no stronger terminal universality claim may rely on this anti-drift closeout alone; the later canonical machine closure bundle must still be named explicitly by digest.",
        ),
        claim_block(
            "plugin_platform_publication_requires_closure_bundle",
            "no stronger plugin-platform publication claim may rely on this closeout alone; operator/internal capability remains bounded until the closure bundle and later publication work say otherwise.",
        ),
        claim_block(
            "arbitrary_software_capability_requires_closure_bundle",
            "no arbitrary software capability claim may be inferred from anti-drift closeout, platform closeout, or weighted controller evidence without the later closure bundle and its stronger terminal contract.",
        ),
    ];

    TassadarPostArticleAntiDriftStabilityCloseoutContract {
        schema_version: 1,
        contract_id: String::from(CONTRACT_ID),
        machine_identity_id: String::from(MACHINE_IDENTITY_ID),
        canonical_architecture_anchor_crate: String::from(CANONICAL_ARCHITECTURE_ANCHOR_CRATE),
        canonical_architecture_boundary_ref: String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        required_surface_rows,
        invalidation_laws,
        stronger_claim_blocks,
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        claim_boundary: String::from(
            "this transformer-owned contract freezes the anti-drift stability closeout only. It says the published computational model, canonical machine identity, control-plane provenance, execution-semantics transport, continuation boundary, fast-route carrier, equivalent-choice neutrality, downward non-influence, portability/minimality, rebased verdict split, and bounded plugin-platform boundary must stay locked to one canonical post-article machine before any stronger terminal claim is considered. It does not itself publish the final claim-bearing canonical machine closure bundle, plugin publication, served/public universality, or arbitrary software capability.",
        ),
        summary: String::from(
            "Transformer anti-drift stability closeout contract freezes 12 required surface locks, 8 invalidation laws, and 3 stronger-claim blocks above one canonical post-article machine while keeping the final closure bundle separate for TAS-215.",
        ),
    }
}

fn required_surface_row(
    surface_id: &str,
    surface_class: TassadarPostArticleAntiDriftSurfaceClass,
    detail: &str,
) -> TassadarPostArticleAntiDriftRequiredSurfaceRow {
    TassadarPostArticleAntiDriftRequiredSurfaceRow {
        surface_id: String::from(surface_id),
        surface_class,
        detail: String::from(detail),
    }
}

fn invalidation_law(
    invalidation_id: &str,
    detail: &str,
) -> TassadarPostArticleAntiDriftInvalidationLaw {
    TassadarPostArticleAntiDriftInvalidationLaw {
        invalidation_id: String::from(invalidation_id),
        detail: String::from(detail),
    }
}

fn claim_block(claim_id: &str, detail: &str) -> TassadarPostArticleAntiDriftClaimBlockRow {
    TassadarPostArticleAntiDriftClaimBlockRow {
        claim_id: String::from(claim_id),
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_anti_drift_stability_closeout_contract,
        TassadarPostArticleAntiDriftSurfaceClass,
    };

    #[test]
    fn anti_drift_contract_keeps_required_surface_and_invalidation_sets_explicit() {
        let contract = build_tassadar_post_article_anti_drift_stability_closeout_contract();

        assert_eq!(
            contract.contract_id,
            "tassadar.post_article_anti_drift_stability_closeout.contract.v1"
        );
        assert_eq!(
            contract.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(contract.required_surface_rows.len(), 12);
        assert_eq!(
            contract.required_surface_rows[0].surface_class,
            TassadarPostArticleAntiDriftSurfaceClass::RuntimeStatement
        );
        assert_eq!(contract.invalidation_laws.len(), 8);
        assert_eq!(contract.stronger_claim_blocks.len(), 3);
        assert_eq!(contract.closure_bundle_issue_id, "TAS-215");
    }
}
