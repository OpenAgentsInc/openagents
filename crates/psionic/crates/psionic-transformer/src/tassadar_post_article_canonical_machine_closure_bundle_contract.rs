use serde::{Deserialize, Serialize};

const CONTRACT_ID: &str = "tassadar.post_article_canonical_machine_closure_bundle.contract.v1";
const MACHINE_IDENTITY_ID: &str = "tassadar.post_article_universality_bridge.machine_identity.v1";
const CANONICAL_ARCHITECTURE_ANCHOR_CRATE: &str = "psionic-transformer";
const CANONICAL_ARCHITECTURE_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";
const NEXT_ISSUE_ID: &str = "TAS-217";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass {
    ProofCarrying,
    Audit,
    RuntimeStatement,
    RuntimeContract,
    BoundaryContract,
    IdentityLock,
    CapabilityBoundary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleArtifactClassRow {
    pub artifact_id: String,
    pub evidence_class: TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleSubjectFieldRow {
    pub field_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleInvalidationLaw {
    pub invalidation_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleClaimInheritanceRow {
    pub claim_surface_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalMachineClosureBundleContract {
    pub schema_version: u16,
    pub contract_id: String,
    pub machine_identity_id: String,
    pub canonical_architecture_anchor_crate: String,
    pub canonical_architecture_boundary_ref: String,
    pub artifact_classification_rows:
        Vec<TassadarPostArticleCanonicalMachineClosureBundleArtifactClassRow>,
    pub required_subject_field_rows:
        Vec<TassadarPostArticleCanonicalMachineClosureBundleSubjectFieldRow>,
    pub invalidation_laws: Vec<TassadarPostArticleCanonicalMachineClosureBundleInvalidationLaw>,
    pub claim_inheritance_rows:
        Vec<TassadarPostArticleCanonicalMachineClosureBundleClaimInheritanceRow>,
    pub closure_bundle_issue_id: String,
    pub next_issue_id: String,
    pub claim_boundary: String,
    pub summary: String,
}

#[must_use]
pub fn build_tassadar_post_article_canonical_machine_closure_bundle_contract(
) -> TassadarPostArticleCanonicalMachineClosureBundleContract {
    let artifact_classification_rows = vec![
        artifact_class_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::RuntimeContract,
            "the historical TCM.v1 runtime contract remains the continuation and effect-carrier contract bound into the canonical closure object.",
        ),
        artifact_class_row(
            "historical_universal_machine_proof",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::ProofCarrying,
            "the historical universal-machine proof remains one proof-bearing ingredient of the closure object rather than a free-floating theoretical premise.",
        ),
        artifact_class_row(
            "historical_universality_witness_suite",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::ProofCarrying,
            "the witness suite remains one proof-bearing ingredient of the closure object so exact witness families cannot drift away from the rebased route identity.",
        ),
        artifact_class_row(
            "historical_universality_verdict_split",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::Audit,
            "the historical verdict split remains one cited audit input so later rebasing keeps the original theory/operator boundary explicit.",
        ),
        artifact_class_row(
            "canonical_machine_identity_lock",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::IdentityLock,
            "the canonical machine lock contributes the tuple-bound machine subject that every proof, audit, and later claim must inherit.",
        ),
        artifact_class_row(
            "canonical_computational_model_statement",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::RuntimeStatement,
            "the published computational-model statement contributes the compute identity and plugin-above-machine boundary for the bundle subject.",
        ),
        artifact_class_row(
            "control_plane_decision_provenance_proof",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::ProofCarrying,
            "the control-plane provenance proof contributes determinism, equivalent-choice, failure semantics, time semantics, information-boundary, hidden-state, and observer rules.",
        ),
        artifact_class_row(
            "execution_semantics_proof_transport_audit",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::Audit,
            "the proof-transport audit contributes the rebased proof boundary that keeps the historical universal-machine proof attached to the owned route.",
        ),
        artifact_class_row(
            "continuation_non_computationality_contract",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::BoundaryContract,
            "the continuation contract contributes the state-carrier boundary that keeps continuation surfaces from becoming a second machine.",
        ),
        artifact_class_row(
            "fast_route_legitimacy_and_carrier_binding_contract",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::BoundaryContract,
            "the fast-route contract contributes the direct-versus-resumable carrier split and quarantines unproven fast families outside the machine.",
        ),
        artifact_class_row(
            "equivalent_choice_neutrality_and_admissibility_contract",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::BoundaryContract,
            "the equivalent-choice contract contributes the admissible divergence boundary used by the control-plane determinism class.",
        ),
        artifact_class_row(
            "downward_non_influence_and_served_conformance_contract",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::BoundaryContract,
            "the downward non-influence contract contributes the lower-plane truth rewrite refusal and fail-closed served envelope.",
        ),
        artifact_class_row(
            "universality_portability_minimality_matrix",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::Audit,
            "the portability/minimality matrix contributes the declared machine matrix and route minimality posture that the bundle must keep explicit.",
        ),
        artifact_class_row(
            "plugin_charter_authority_boundary",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::CapabilityBoundary,
            "the plugin charter contributes the observer, governance, and state-class split that keep the plugin layer above the same canonical machine.",
        ),
        artifact_class_row(
            "anti_drift_stability_closeout",
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::Audit,
            "the anti-drift closeout contributes the explicit statement that compute, control, continuation, capability, and served posture are already locked to one machine before the closure bundle is published.",
        ),
    ];

    let required_subject_field_rows = vec![
        subject_field_row(
            "machine_identity_tuple",
            "the closure object must bind one canonical tuple over machine id, model id, weight digests, route id, route digest, and continuation contract digests.",
        ),
        subject_field_row(
            "computational_model_statement",
            "the closure object must name the separately published computational-model statement instead of restating compute identity implicitly.",
        ),
        subject_field_row(
            "determinism_contract",
            "the closure object must centralize the selected determinism class, replay posture, and equivalent-choice relation.",
        ),
        subject_field_row(
            "control_plane_proof",
            "the closure object must bind branch, retry, and stop decisions to model outputs and canonical machine identity by digest.",
        ),
        subject_field_row(
            "execution_semantics_transport",
            "the closure object must bind the proof-transport audit so the historical proof stays attached to the rebased route.",
        ),
        subject_field_row(
            "carrier_split",
            "the closure object must bind the explicit direct-versus-resumable carrier split instead of allowing mixed-carrier recomposition.",
        ),
        subject_field_row(
            "continuation_boundary",
            "the closure object must bind the continuation contract and admitted continuation surfaces to the same machine.",
        ),
        subject_field_row(
            "state_classes_and_hidden_state_closure",
            "the closure object must bind admitted state classes and the hidden-state closure instead of letting later workflow surfaces smuggle new compute.",
        ),
        subject_field_row(
            "failure_lattice_and_time_semantics",
            "the closure object must bind the failure lattice and time semantics used by the replay-stable control proof.",
        ),
        subject_field_row(
            "information_and_training_boundaries",
            "the closure object must bind information-boundary and training-versus-inference posture so those boundaries cannot drift across later claims.",
        ),
        subject_field_row(
            "observer_model",
            "the closure object must bind the observer model and acceptance requirements that distinguish proof-bearing from sampled or anecdotal evidence.",
        ),
        subject_field_row(
            "portability_and_minimality",
            "the closure object must bind portability and minimality posture so route-class and machine-class drift stay explicit.",
        ),
        subject_field_row(
            "proof_vs_audit_classification",
            "the closure object must classify proofs, audits, and boundary contracts explicitly rather than letting later prose blur those roles.",
        ),
    ];

    let invalidation_laws = vec![
        invalidation_law(
            "route_drift_rejected",
            "route drift remains invalid: the canonical route id and route descriptor digest must stay identical across the bound proof-bearing and audit-bearing surfaces.",
        ),
        invalidation_law(
            "determinism_mismatch_rejected",
            "determinism mismatch remains invalid: the selected determinism class, replay posture, and equivalent-choice relation must stay bundle-bound.",
        ),
        invalidation_law(
            "hidden_state_channel_rejected",
            "hidden state channel drift remains invalid: hidden workflow, spill, session, process-object, or plugin surfaces may not reopen undeclared state channels.",
        ),
        invalidation_law(
            "fast_route_substitution_rejected",
            "fast-route substitution remains invalid: unproven route families may not be treated as the canonical machine by inheritance.",
        ),
        invalidation_law(
            "downward_influence_rejected",
            "downward influence remains invalid: later plugin, publication, or served layers may not rewrite lower-plane truth or machine identity.",
        ),
        invalidation_law(
            "observer_model_mismatch_rejected",
            "observer mismatch remains invalid: sampled or anecdotal observation cannot replace the bundle-bound verifier and acceptance model.",
        ),
        invalidation_law(
            "portability_failure_rejected",
            "portability failure remains invalid: machine-class posture and route portability must stay the same as the declared matrix.",
        ),
        invalidation_law(
            "minimality_failure_rejected",
            "minimality failure remains invalid: route-carrier and component-boundary minimality must stay explicit on the canonical machine.",
        ),
        invalidation_law(
            "bundle_digest_mismatch_rejected",
            "bundle digest mismatch remains invalid: downstream terminal, controller, receipt, publication, or platform claims must inherit the declared closure bundle by digest instead of silently recomposing the machine.",
        ),
    ];

    let claim_inheritance_rows = vec![
        claim_inheritance_row(
            "terminal_universality_claim",
            "any terminal universality claim must cite the canonical machine closure bundle by digest instead of inferring the machine from adjacent rebasing and anti-drift artifacts.",
        ),
        claim_inheritance_row(
            "weighted_plugin_controller_claim",
            "any weighted plugin controller claim must inherit this bundle by digest so controller ownership cannot float onto a weaker or different machine.",
        ),
        claim_inheritance_row(
            "plugin_invocation_receipt_claim",
            "any plugin invocation receipt or replay-class claim must inherit this bundle by digest so receipt identity remains tied to the same machine subject.",
        ),
        claim_inheritance_row(
            "plugin_publication_claim",
            "any plugin publication or trust-tier claim must inherit this bundle by digest so publication posture cannot silently redefine the machine.",
        ),
        claim_inheritance_row(
            "plugin_platform_claim",
            "any bounded platform claim must inherit this bundle by digest so operator/internal capability stays bound to one canonical machine.",
        ),
    ];

    TassadarPostArticleCanonicalMachineClosureBundleContract {
        schema_version: 1,
        contract_id: String::from(CONTRACT_ID),
        machine_identity_id: String::from(MACHINE_IDENTITY_ID),
        canonical_architecture_anchor_crate: String::from(CANONICAL_ARCHITECTURE_ANCHOR_CRATE),
        canonical_architecture_boundary_ref: String::from(CANONICAL_ARCHITECTURE_BOUNDARY_REF),
        artifact_classification_rows,
        required_subject_field_rows,
        invalidation_laws,
        claim_inheritance_rows,
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        next_issue_id: String::from(NEXT_ISSUE_ID),
        claim_boundary: String::from(
            "this transformer-owned contract freezes the final claim-bearing canonical machine closure-bundle boundary. It requires one digest-bound closure object to bind the published computational model, canonical machine tuple, determinism contract, control-plane proof, execution-semantics transport, continuation boundary, carrier split, state classes, hidden-state closure, failure lattice, observer model, portability/minimality posture, and explicit proof-vs-audit classification into one indivisible machine identity. It does not itself publish plugin catalogs, plugin publication, served/public universality, or arbitrary software capability.",
        ),
        summary: String::from(
            "Transformer canonical machine closure-bundle contract freezes 15 artifact classifications, 13 required subject fields, 9 invalidation laws, and 5 downstream inheritance requirements above one digest-bound post-article machine.",
        ),
    }
}

fn artifact_class_row(
    artifact_id: &str,
    evidence_class: TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineClosureBundleArtifactClassRow {
    TassadarPostArticleCanonicalMachineClosureBundleArtifactClassRow {
        artifact_id: String::from(artifact_id),
        evidence_class,
        detail: String::from(detail),
    }
}

fn subject_field_row(
    field_id: &str,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineClosureBundleSubjectFieldRow {
    TassadarPostArticleCanonicalMachineClosureBundleSubjectFieldRow {
        field_id: String::from(field_id),
        detail: String::from(detail),
    }
}

fn invalidation_law(
    invalidation_id: &str,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineClosureBundleInvalidationLaw {
    TassadarPostArticleCanonicalMachineClosureBundleInvalidationLaw {
        invalidation_id: String::from(invalidation_id),
        detail: String::from(detail),
    }
}

fn claim_inheritance_row(
    claim_surface_id: &str,
    detail: &str,
) -> TassadarPostArticleCanonicalMachineClosureBundleClaimInheritanceRow {
    TassadarPostArticleCanonicalMachineClosureBundleClaimInheritanceRow {
        claim_surface_id: String::from(claim_surface_id),
        detail: String::from(detail),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_machine_closure_bundle_contract,
        TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass,
    };

    #[test]
    fn closure_bundle_contract_keeps_machine_subject_and_inheritance_requirements_explicit() {
        let contract = build_tassadar_post_article_canonical_machine_closure_bundle_contract();

        assert_eq!(
            contract.contract_id,
            "tassadar.post_article_canonical_machine_closure_bundle.contract.v1"
        );
        assert_eq!(
            contract.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(contract.artifact_classification_rows.len(), 15);
        assert_eq!(
            contract.artifact_classification_rows[0].evidence_class,
            TassadarPostArticleCanonicalMachineClosureBundleEvidenceClass::RuntimeContract
        );
        assert_eq!(contract.required_subject_field_rows.len(), 13);
        assert_eq!(contract.invalidation_laws.len(), 9);
        assert_eq!(contract.claim_inheritance_rows.len(), 5);
        assert_eq!(contract.closure_bundle_issue_id, "TAS-215");
        assert_eq!(contract.next_issue_id, "TAS-217");
    }
}
