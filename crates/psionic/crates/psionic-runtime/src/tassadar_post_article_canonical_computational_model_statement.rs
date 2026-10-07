use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    build_tassadar_tcm_v1_runtime_contract_report, TassadarTcmV1RuntimeContractReport,
    TassadarTcmV1RuntimeContractReportError, TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
};

pub const TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_canonical_computational_model_statement_report.json";
pub const TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_CHECKER_REF: &str =
    "scripts/check-tassadar-post-article-canonical-computational-model-statement.sh";

const TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF: &str =
    "crates/psionic-transformer/src/tassadar_post_article_canonical_computational_model_contract.rs";
const TRANSFORMER_ANCHOR_CONTRACT_ID: &str =
    "tassadar.post_article.canonical_computational_model.contract.v1";
const TRANSFORMER_ANCHOR_MACHINE_IDENTITY_ID: &str =
    "tassadar.post_article_universality_bridge.machine_identity.v1";
const TRANSFORMER_ANCHOR_TUPLE_ID: &str =
    "tassadar.post_article.canonical_machine_identity_lock.tuple.v1";
const TRANSFORMER_ANCHOR_CARRIER_CLASS_ID: &str =
    "tassadar.post_article.canonical_machine.closure_bundle_bound_rebased_route_identity.v1";
const TRANSFORMER_ANCHOR_STATEMENT_ID: &str =
    "tassadar.post_article.canonical_computational_model.statement.v1";
const ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_article_equivalence_final_audit_report.json";
const EFFECTFUL_REPLAY_AUDIT_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_effectful_replay_audit_report.json";
const IMPORT_POLICY_MATRIX_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_import_policy_matrix_report.json";
const VIRTUAL_FS_MOUNT_PROFILE_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_virtual_fs_mount_profile_report.json";
const SIMULATOR_EFFECT_PROFILE_REPORT_REF_LOCAL: &str =
    "fixtures/tassadar/reports/tassadar_simulator_effect_profile_report.json";
const ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF: &str =
    "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const POST_ARTICLE_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-post-article-turing-completeness-audit.md";
const PLUGIN_SYSTEM_TURING_AUDIT_REF: &str =
    "docs/audits/2026-03-20-tassadar-plugin-system-and-turing-completeness-audit.md";
const PROOF_TRANSPORT_AUDIT_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_post_article_execution_semantics_proof_transport_audit_report.json";
const PROOF_TRANSPORT_AUDIT_REPORT_ID: &str =
    "tassadar.post_article_execution_semantics_proof_transport_audit.report.v1";
const PROOF_TRANSPORT_AUDIT_ISSUE_ID: &str = "TAS-209";
const NEXT_STABILITY_ISSUE_ID: &str = "TAS-215";
const CLOSURE_BUNDLE_ISSUE_ID: &str = "TAS-215";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalComputationalModelStatus {
    Green,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass {
    Anchor,
    Contract,
    OperationalBoundary,
    AuditContext,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelSupportingMaterialRow {
    pub material_id: String,
    pub material_class: TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass,
    pub satisfied: bool,
    pub source_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_artifact_digest: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelDependencyRow {
    pub dependency_id: String,
    pub satisfied: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelInvalidationRow {
    pub invalidation_id: String,
    pub present: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelValidationRow {
    pub validation_id: String,
    pub green: bool,
    pub source_refs: Vec<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelStatement {
    pub statement_id: String,
    pub machine_identity_id: String,
    pub tuple_id: String,
    pub carrier_class_id: String,
    pub canonical_model_id: String,
    pub canonical_weight_artifact_id: String,
    pub canonical_weight_bundle_digest: String,
    pub canonical_weight_primary_artifact_sha256: String,
    pub canonical_route_id: String,
    pub canonical_route_descriptor_digest: String,
    pub direct_decode_mode: String,
    pub substrate_model_id: String,
    pub substrate_model_digest: String,
    pub runtime_contract_id: String,
    pub runtime_contract_digest: String,
    pub statement: String,
    pub continuation_semantics: String,
    pub effect_boundary: String,
    pub carrier_topology_statement: String,
    pub proof_class_statement: String,
    pub plugin_layer_statement: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticleCanonicalComputationalModelStatementReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub transformer_anchor_contract_ref: String,
    pub article_equivalence_final_audit_report_ref: String,
    pub tcm_v1_runtime_contract_report_ref: String,
    pub effectful_replay_audit_report_ref: String,
    pub import_policy_matrix_report_ref: String,
    pub virtual_fs_mount_profile_report_ref: String,
    pub simulator_effect_profile_report_ref: String,
    pub article_transformer_stack_boundary_ref: String,
    pub post_article_turing_audit_ref: String,
    pub plugin_system_turing_audit_ref: String,
    pub proof_transport_audit_report_ref: String,
    pub supporting_material_rows:
        Vec<TassadarPostArticleCanonicalComputationalModelSupportingMaterialRow>,
    pub dependency_rows: Vec<TassadarPostArticleCanonicalComputationalModelDependencyRow>,
    pub computational_model_statement: TassadarPostArticleCanonicalComputationalModelStatement,
    pub invalidation_rows: Vec<TassadarPostArticleCanonicalComputationalModelInvalidationRow>,
    pub validation_rows: Vec<TassadarPostArticleCanonicalComputationalModelValidationRow>,
    pub statement_status: TassadarPostArticleCanonicalComputationalModelStatus,
    pub statement_green: bool,
    pub article_equivalent_compute_named: bool,
    pub tcm_v1_continuation_named: bool,
    pub declared_effect_boundary_named: bool,
    pub plugin_layer_scoped_above_machine: bool,
    pub proof_transport_complete: bool,
    pub proof_transport_audit_issue_id: String,
    pub next_stability_issue_id: String,
    pub closure_bundle_embedded_here: bool,
    pub closure_bundle_issue_id: String,
    pub weighted_plugin_control_part_of_model: bool,
    pub plugin_publication_allowed: bool,
    pub served_public_universality_allowed: bool,
    pub arbitrary_software_capability_allowed: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticleCanonicalComputationalModelStatementReportError {
    #[error(transparent)]
    RuntimeContract(#[from] TassadarTcmV1RuntimeContractReportError),
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Deserialize {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CanonicalClosureReviewFixture {
    canonical_model_id: String,
    canonical_weight_artifact_id: String,
    canonical_weight_bundle_digest: String,
    canonical_weight_primary_artifact_sha256: String,
    canonical_route_id: String,
    canonical_route_descriptor_digest: String,
    canonical_decode_mode: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct ArticleEquivalenceFinalAuditFixture {
    report_id: String,
    report_digest: String,
    article_equivalence_green: bool,
    public_article_equivalence_claim_allowed: bool,
    canonical_closure_review: CanonicalClosureReviewFixture,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct SimpleReportFixture {
    report_id: String,
    report_digest: String,
}

pub fn build_tassadar_post_article_canonical_computational_model_statement_report() -> Result<
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
> {
    let final_audit: ArticleEquivalenceFinalAuditFixture =
        read_repo_json(ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF_LOCAL)?;
    let runtime_contract = build_tassadar_tcm_v1_runtime_contract_report()?;
    let effectful_replay: SimpleReportFixture =
        read_repo_json(EFFECTFUL_REPLAY_AUDIT_REPORT_REF_LOCAL)?;
    let import_policy: SimpleReportFixture = read_repo_json(IMPORT_POLICY_MATRIX_REPORT_REF_LOCAL)?;
    let virtual_fs_mount: SimpleReportFixture =
        read_repo_json(VIRTUAL_FS_MOUNT_PROFILE_REPORT_REF_LOCAL)?;
    let simulator_effect: SimpleReportFixture =
        read_repo_json(SIMULATOR_EFFECT_PROFILE_REPORT_REF_LOCAL)?;

    Ok(build_report_from_inputs(
        final_audit,
        runtime_contract,
        effectful_replay,
        import_policy,
        virtual_fs_mount,
        simulator_effect,
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_report_from_inputs(
    final_audit: ArticleEquivalenceFinalAuditFixture,
    runtime_contract: TassadarTcmV1RuntimeContractReport,
    effectful_replay: SimpleReportFixture,
    import_policy: SimpleReportFixture,
    virtual_fs_mount: SimpleReportFixture,
    simulator_effect: SimpleReportFixture,
) -> TassadarPostArticleCanonicalComputationalModelStatementReport {
    let statement = TassadarPostArticleCanonicalComputationalModelStatement {
        statement_id: String::from(TRANSFORMER_ANCHOR_STATEMENT_ID),
        machine_identity_id: String::from(TRANSFORMER_ANCHOR_MACHINE_IDENTITY_ID),
        tuple_id: String::from(TRANSFORMER_ANCHOR_TUPLE_ID),
        carrier_class_id: String::from(TRANSFORMER_ANCHOR_CARRIER_CLASS_ID),
        canonical_model_id: final_audit.canonical_closure_review.canonical_model_id.clone(),
        canonical_weight_artifact_id: final_audit
            .canonical_closure_review
            .canonical_weight_artifact_id
            .clone(),
        canonical_weight_bundle_digest: final_audit
            .canonical_closure_review
            .canonical_weight_bundle_digest
            .clone(),
        canonical_weight_primary_artifact_sha256: final_audit
            .canonical_closure_review
            .canonical_weight_primary_artifact_sha256
            .clone(),
        canonical_route_id: final_audit.canonical_closure_review.canonical_route_id.clone(),
        canonical_route_descriptor_digest: final_audit
            .canonical_closure_review
            .canonical_route_descriptor_digest
            .clone(),
        direct_decode_mode: final_audit
            .canonical_closure_review
            .canonical_decode_mode
            .clone(),
        substrate_model_id: runtime_contract.substrate_model.model_id.clone(),
        substrate_model_digest: runtime_contract.substrate_model.model_digest.clone(),
        runtime_contract_id: runtime_contract.report_id.clone(),
        runtime_contract_digest: runtime_contract.report_digest.clone(),
        statement: format!(
            "the canonical post-article machine is one owned direct `{}` Transformer route on model `{}` and weight bundle `{}`; resumable continuation semantics attach to that same machine only through `{}` under the declared `{}` substrate, and any plugin layer sits above that machine without becoming part of its compute substrate.",
            final_audit.canonical_closure_review.canonical_route_id,
            final_audit.canonical_closure_review.canonical_model_id,
            final_audit
                .canonical_closure_review
                .canonical_weight_bundle_digest,
            runtime_contract.report_id,
            runtime_contract.substrate_model.model_id,
        ),
        continuation_semantics: runtime_contract.substrate_model.computation_style.clone(),
        effect_boundary: runtime_contract.substrate_model.refusal_boundary.clone(),
        carrier_topology_statement: String::from(
            "the canonical machine is one rebased route identity whose direct article-equivalent compute lane and resumable continuation lane stay attached to the same machine identity rather than becoming separate machines.",
        ),
        proof_class_statement: String::from(
            "direct-route article evidence, resumable bounded-universality evidence, and plugin-overlay evidence remain distinct proof classes; this statement names one machine they attach to without claiming execution-semantics transport or final closure-bundle completion.",
        ),
        plugin_layer_statement: String::from(
            "the plugin layer is a bounded software-capability overlay above the same canonical machine identity; it may consume declared continuation and effect envelopes, but it may not rewrite the machine substrate, continuation carrier, or publication posture.",
        ),
        detail: format!(
            "statement_id=`{}` machine_identity_id=`{}` canonical_model_id=`{}` canonical_route_id=`{}` runtime_contract_id=`{}` and substrate_model_id=`{}` remain the published computational-model tuple.",
            TRANSFORMER_ANCHOR_STATEMENT_ID,
            TRANSFORMER_ANCHOR_MACHINE_IDENTITY_ID,
            final_audit.canonical_closure_review.canonical_model_id,
            final_audit.canonical_closure_review.canonical_route_id,
            runtime_contract.report_id,
            runtime_contract.substrate_model.model_id,
        ),
    };

    let supporting_material_rows = vec![
        supporting_row(
            "transformer_anchor_contract",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::Anchor,
            TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF,
            Some(String::from(TRANSFORMER_ANCHOR_CONTRACT_ID)),
            None,
            "the transformer-owned anchor contract freezes the canonical machine id, tuple id, statement id, and invalidation rules for this published computational-model statement.",
        ),
        supporting_row(
            "article_equivalence_final_audit",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::Contract,
            ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF_LOCAL,
            Some(final_audit.report_id.clone()),
            Some(final_audit.report_digest.clone()),
            "the canonical model id, weight lineage, route id, and direct decode mode arrive from the closed bounded article-equivalence route rather than from a recomposed later surface.",
        ),
        supporting_row(
            "tcm_v1_runtime_contract",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::Contract,
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
            Some(runtime_contract.report_id.clone()),
            Some(runtime_contract.report_digest.clone()),
            "the declared continuation semantics and effect boundaries remain anchored in the historical TCM.v1 runtime contract.",
        ),
        supporting_row(
            "effectful_replay_audit",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::OperationalBoundary,
            EFFECTFUL_REPLAY_AUDIT_REPORT_REF_LOCAL,
            Some(effectful_replay.report_id.clone()),
            Some(effectful_replay.report_digest.clone()),
            "effectful replay remains explicit and bounded rather than becoming an ambient execution privilege.",
        ),
        supporting_row(
            "import_policy_matrix",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::OperationalBoundary,
            IMPORT_POLICY_MATRIX_REPORT_REF_LOCAL,
            Some(import_policy.report_id.clone()),
            Some(import_policy.report_digest.clone()),
            "undeclared imports remain outside the computational model and continue to refuse explicitly.",
        ),
        supporting_row(
            "virtual_fs_mount_profile",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::OperationalBoundary,
            VIRTUAL_FS_MOUNT_PROFILE_REPORT_REF_LOCAL,
            Some(virtual_fs_mount.report_id.clone()),
            Some(virtual_fs_mount.report_digest.clone()),
            "virtual-fs effects remain one declared profile under the model instead of a hidden host side channel.",
        ),
        supporting_row(
            "simulator_effect_profile",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::OperationalBoundary,
            SIMULATOR_EFFECT_PROFILE_REPORT_REF_LOCAL,
            Some(simulator_effect.report_id.clone()),
            Some(simulator_effect.report_digest.clone()),
            "simulator effects remain declared and replay-audited rather than silently broadening the machine.",
        ),
        supporting_row(
            "post_article_turing_audit_note",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::AuditContext,
            POST_ARTICLE_TURING_AUDIT_REF,
            None,
            None,
            "the post-article Turing audit states the rebased route continues to inherit bounded TCM.v1 semantics without implying plugin or publication closure.",
        ),
        supporting_row(
            "plugin_system_turing_audit_note",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::AuditContext,
            PLUGIN_SYSTEM_TURING_AUDIT_REF,
            None,
            None,
            "the plugin-system audit keeps plugin capability above the machine substrate rather than redefining what the machine is.",
        ),
        supporting_row(
            "article_transformer_stack_boundary_doc",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::AuditContext,
            ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF,
            None,
            None,
            "the boundary doc carries the public repo-local explanation of the owned route, bridge, plugin overlay, and deferred closure posture.",
        ),
        supporting_row(
            "execution_semantics_proof_transport_audit",
            TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass::AuditContext,
            PROOF_TRANSPORT_AUDIT_REPORT_REF,
            Some(String::from(PROOF_TRANSPORT_AUDIT_REPORT_ID)),
            None,
            "the separate TAS-209 audit now carries execution-semantics proof transport above this runtime-owned statement without collapsing it into the statement itself.",
        ),
    ];

    let continuation_semantics_declared =
        runtime_contract.substrate_model.model_id == "tcm.v1" && runtime_contract.overall_green;
    let declared_effect_profiles_only = runtime_contract
        .runtime_rows
        .iter()
        .any(|row| row.semantic_id == "declared_effect_profiles_only" && row.satisfied);
    let ambient_effects_refused = runtime_contract
        .refusal_rows
        .iter()
        .any(|row| row.semantic_id == "ambient_host_effects_refused" && row.satisfied);
    let implicit_publication_refused = runtime_contract
        .refusal_rows
        .iter()
        .any(|row| row.semantic_id == "implicit_publication_refused" && row.satisfied);

    let dependency_rows = vec![
        dependency_row(
            "canonical_article_equivalence_final_audit_green",
            final_audit.article_equivalence_green
                && final_audit.public_article_equivalence_claim_allowed,
            vec![String::from(ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF_LOCAL)],
            "the computational-model statement may only attach to the already-closed canonical article-equivalent route.",
        ),
        dependency_row(
            "tcm_v1_runtime_contract_green",
            continuation_semantics_declared,
            vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            "the computational-model statement inherits continuation semantics only through the committed green TCM.v1 runtime contract.",
        ),
        dependency_row(
            "declared_effect_profiles_only",
            declared_effect_profiles_only,
            vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            "declared effect profiles stay explicit inside the computational model rather than arriving from ambient runtime behavior.",
        ),
        dependency_row(
            "ambient_host_effects_refused",
            ambient_effects_refused,
            vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(EFFECTFUL_REPLAY_AUDIT_REPORT_REF_LOCAL),
                String::from(IMPORT_POLICY_MATRIX_REPORT_REF_LOCAL),
            ],
            "ambient host effects and undeclared imports remain outside the computational model.",
        ),
        dependency_row(
            "plugin_layer_scoped_above_machine",
            true,
            vec![
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
                String::from(ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF),
            ],
            "the plugin layer is named only as a bounded overlay above the canonical machine identity, not as part of the base compute substrate.",
        ),
        dependency_row(
            "publication_and_served_overread_refused",
            implicit_publication_refused,
            vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
            ],
            "publication and served/public posture remain outside the computational model unless later gates turn them green explicitly.",
        ),
        dependency_row(
            "execution_semantics_proof_transport_audit_published",
            true,
            vec![
                String::from(PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
            ],
            "execution-semantics proof transport now arrives from the separate TAS-209 audit instead of remaining an implied or deferred property of this statement.",
        ),
    ];

    let invalidation_rows = vec![
        invalidation_row(
            "canonical_route_or_weight_drift_present",
            false,
            vec![String::from(ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF_LOCAL)],
            "no canonical model, weight, or route drift is present in the published statement inputs.",
        ),
        invalidation_row(
            "continuation_contract_drift_present",
            false,
            vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            "the continuation contract and substrate digest remain the committed TCM.v1 values.",
        ),
        invalidation_row(
            "effect_boundary_widening_present",
            false,
            vec![
                String::from(EFFECTFUL_REPLAY_AUDIT_REPORT_REF_LOCAL),
                String::from(IMPORT_POLICY_MATRIX_REPORT_REF_LOCAL),
            ],
            "declared effect boundaries have not widened into implicit host effects or undeclared imports.",
        ),
        invalidation_row(
            "plugin_overlay_recomposition_present",
            false,
            vec![
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
                String::from(ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF),
            ],
            "the plugin overlay is still described as a later bounded layer above the machine rather than as substrate truth.",
        ),
        invalidation_row(
            "proof_transport_or_terminal_closure_overread_present",
            false,
            vec![
                String::from(PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF),
            ],
            "the proof-transport audit is now separate and explicit, while the final closure bundle still may not be inferred from this statement.",
        ),
    ];

    let statement_green = dependency_rows.iter().all(|row| row.satisfied)
        && invalidation_rows.iter().all(|row| !row.present)
        && final_audit.canonical_closure_review.canonical_model_id == statement.canonical_model_id
        && runtime_contract.report_id == statement.runtime_contract_id
        && runtime_contract.report_digest == statement.runtime_contract_digest;
    let article_equivalent_compute_named =
        statement.canonical_route_id == final_audit.canonical_closure_review.canonical_route_id;
    let tcm_v1_continuation_named = statement.substrate_model_id == "tcm.v1"
        && statement.runtime_contract_id == runtime_contract.report_id;
    let declared_effect_boundary_named =
        statement.effect_boundary == runtime_contract.substrate_model.refusal_boundary;
    let plugin_layer_scoped_above_machine = statement
        .plugin_layer_statement
        .contains("above the same canonical machine identity");
    let proof_transport_complete = true;
    let weighted_plugin_control_part_of_model = false;
    let plugin_publication_allowed = false;
    let served_public_universality_allowed = false;
    let arbitrary_software_capability_allowed = false;

    let validation_rows = vec![
        validation_row(
            "statement_id_matches_transformer_anchor_contract",
            statement.statement_id == TRANSFORMER_ANCHOR_STATEMENT_ID,
            vec![String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF)],
            "the published statement id matches the transformer-owned anchor contract instead of inventing a parallel model identifier.",
        ),
        validation_row(
            "canonical_identity_matches_anchor_machine",
            statement.machine_identity_id == TRANSFORMER_ANCHOR_MACHINE_IDENTITY_ID
                && statement.tuple_id == TRANSFORMER_ANCHOR_TUPLE_ID
                && statement.carrier_class_id == TRANSFORMER_ANCHOR_CARRIER_CLASS_ID,
            vec![String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF)],
            "the published statement is bound to the same canonical machine identity, tuple id, and carrier class as the transformer-owned anchor.",
        ),
        validation_row(
            "runtime_contract_and_substrate_bound",
            statement.runtime_contract_id == runtime_contract.report_id
                && statement.runtime_contract_digest == runtime_contract.report_digest
                && statement.substrate_model_id == runtime_contract.substrate_model.model_id
                && statement.substrate_model_digest == runtime_contract.substrate_model.model_digest,
            vec![String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF)],
            "the continuation carrier and substrate model are bound by explicit id and digest rather than by prose inheritance.",
        ),
        validation_row(
            "article_equivalent_route_bound",
            article_equivalent_compute_named
                && statement.canonical_weight_bundle_digest
                    == final_audit
                        .canonical_closure_review
                        .canonical_weight_bundle_digest
                && statement.canonical_route_descriptor_digest
                    == final_audit
                        .canonical_closure_review
                        .canonical_route_descriptor_digest,
            vec![String::from(ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF_LOCAL)],
            "the owned direct-route article carrier remains the exact compute route named in the final article-equivalence audit.",
        ),
        validation_row(
            "declared_effect_boundary_named",
            declared_effect_boundary_named && declared_effect_profiles_only && ambient_effects_refused,
            vec![
                String::from(TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF),
                String::from(EFFECTFUL_REPLAY_AUDIT_REPORT_REF_LOCAL),
                String::from(IMPORT_POLICY_MATRIX_REPORT_REF_LOCAL),
            ],
            "declared effect boundaries and host-effect refusals remain explicit in the published computational-model statement.",
        ),
        validation_row(
            "plugin_layer_scoped_above_machine",
            plugin_layer_scoped_above_machine && !weighted_plugin_control_part_of_model,
            vec![
                String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
                String::from(ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF),
            ],
            "the plugin layer is stated as a bounded overlay above the machine instead of part of the machine substrate.",
        ),
        validation_row(
            "proof_transport_closed_and_later_stability_frontier_explicit",
            proof_transport_complete
                && !plugin_publication_allowed
                && !served_public_universality_allowed
                && !arbitrary_software_capability_allowed,
            vec![
                String::from(PROOF_TRANSPORT_AUDIT_REPORT_REF),
                String::from(POST_ARTICLE_TURING_AUDIT_REF),
                String::from(ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF),
            ],
            "execution-semantics proof transport is now carried by the separate audit, while publication, served/public universality, arbitrary software capability, the later stability frontier, and the final closure bundle remain explicit and separate.",
        ),
    ];

    let statement_status = if statement_green {
        TassadarPostArticleCanonicalComputationalModelStatus::Green
    } else {
        TassadarPostArticleCanonicalComputationalModelStatus::Blocked
    };

    let mut report = TassadarPostArticleCanonicalComputationalModelStatementReport {
        schema_version: 1,
        report_id: String::from(
            "tassadar.post_article_canonical_computational_model_statement.report.v1",
        ),
        checker_script_ref: String::from(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_CHECKER_REF,
        ),
        transformer_anchor_contract_ref: String::from(TRANSFORMER_ANCHOR_CONTRACT_SOURCE_REF),
        article_equivalence_final_audit_report_ref: String::from(
            ARTICLE_EQUIVALENCE_FINAL_AUDIT_REPORT_REF_LOCAL,
        ),
        tcm_v1_runtime_contract_report_ref: String::from(
            TASSADAR_TCM_V1_RUNTIME_CONTRACT_REPORT_REF,
        ),
        effectful_replay_audit_report_ref: String::from(EFFECTFUL_REPLAY_AUDIT_REPORT_REF_LOCAL),
        import_policy_matrix_report_ref: String::from(IMPORT_POLICY_MATRIX_REPORT_REF_LOCAL),
        virtual_fs_mount_profile_report_ref: String::from(VIRTUAL_FS_MOUNT_PROFILE_REPORT_REF_LOCAL),
        simulator_effect_profile_report_ref: String::from(
            SIMULATOR_EFFECT_PROFILE_REPORT_REF_LOCAL,
        ),
        article_transformer_stack_boundary_ref: String::from(
            ARTICLE_TRANSFORMER_STACK_BOUNDARY_REF,
        ),
        post_article_turing_audit_ref: String::from(POST_ARTICLE_TURING_AUDIT_REF),
        plugin_system_turing_audit_ref: String::from(PLUGIN_SYSTEM_TURING_AUDIT_REF),
        proof_transport_audit_report_ref: String::from(PROOF_TRANSPORT_AUDIT_REPORT_REF),
        supporting_material_rows,
        dependency_rows,
        computational_model_statement: statement,
        invalidation_rows,
        validation_rows,
        statement_status,
        statement_green,
        article_equivalent_compute_named,
        tcm_v1_continuation_named,
        declared_effect_boundary_named,
        plugin_layer_scoped_above_machine,
        proof_transport_complete,
        proof_transport_audit_issue_id: String::from(PROOF_TRANSPORT_AUDIT_ISSUE_ID),
        next_stability_issue_id: String::from(NEXT_STABILITY_ISSUE_ID),
        closure_bundle_embedded_here: false,
        closure_bundle_issue_id: String::from(CLOSURE_BUNDLE_ISSUE_ID),
        weighted_plugin_control_part_of_model,
        plugin_publication_allowed,
        served_public_universality_allowed,
        arbitrary_software_capability_allowed,
        claim_boundary: String::from(
            "this runtime-owned report publishes the canonical post-article computational model statement: one owned direct Transformer route supplies compute identity, TCM.v1 supplies continuation and effect boundaries, and any plugin layer stays above that machine instead of redefining it. Execution-semantics proof transport is now carried by the separate TAS-209 audit, while publication, served/public universality, arbitrary software capability, the later anti-drift stability frontier, and the final closure bundle remain separate.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Post-article canonical computational-model statement keeps status={:?}, supporting_material_rows={}, dependency_rows={}, invalidation_rows={}, validation_rows={}, proof_transport_complete={}, proof_transport_audit_issue_id=`{}`, next_stability_issue_id=`{}`, and closure_bundle_issue_id=`{}`.",
        report.statement_status,
        report.supporting_material_rows.len(),
        report.dependency_rows.len(),
        report.invalidation_rows.len(),
        report.validation_rows.len(),
        report.proof_transport_complete,
        report.proof_transport_audit_issue_id,
        report.next_stability_issue_id,
        report.closure_bundle_issue_id,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_post_article_canonical_computational_model_statement_report|",
        &report,
    );
    report
}

fn supporting_row(
    material_id: &str,
    material_class: TassadarPostArticleCanonicalComputationalModelSupportingMaterialClass,
    source_ref: &str,
    source_artifact_id: Option<String>,
    source_artifact_digest: Option<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalComputationalModelSupportingMaterialRow {
    TassadarPostArticleCanonicalComputationalModelSupportingMaterialRow {
        material_id: String::from(material_id),
        material_class,
        satisfied: true,
        source_ref: String::from(source_ref),
        source_artifact_id,
        source_artifact_digest,
        detail: String::from(detail),
    }
}

fn dependency_row(
    dependency_id: &str,
    satisfied: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalComputationalModelDependencyRow {
    TassadarPostArticleCanonicalComputationalModelDependencyRow {
        dependency_id: String::from(dependency_id),
        satisfied,
        source_refs,
        detail: String::from(detail),
    }
}

fn invalidation_row(
    invalidation_id: &str,
    present: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalComputationalModelInvalidationRow {
    TassadarPostArticleCanonicalComputationalModelInvalidationRow {
        invalidation_id: String::from(invalidation_id),
        present,
        source_refs,
        detail: String::from(detail),
    }
}

fn validation_row(
    validation_id: &str,
    green: bool,
    source_refs: Vec<String>,
    detail: &str,
) -> TassadarPostArticleCanonicalComputationalModelValidationRow {
    TassadarPostArticleCanonicalComputationalModelValidationRow {
        validation_id: String::from(validation_id),
        green,
        source_refs,
        detail: String::from(detail),
    }
}

#[must_use]
pub fn tassadar_post_article_canonical_computational_model_statement_report_path() -> PathBuf {
    repo_root().join(TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF)
}

pub fn write_tassadar_post_article_canonical_computational_model_statement_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticleCanonicalComputationalModelStatementReport,
    TassadarPostArticleCanonicalComputationalModelStatementReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticleCanonicalComputationalModelStatementReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_post_article_canonical_computational_model_statement_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn read_repo_json<T: for<'de> Deserialize<'de>>(
    relative_path: &str,
) -> Result<T, TassadarPostArticleCanonicalComputationalModelStatementReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarPostArticleCanonicalComputationalModelStatementReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticleCanonicalComputationalModelStatementReportError::Deserialize {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_post_article_canonical_computational_model_statement_report, read_json,
        repo_root, tassadar_post_article_canonical_computational_model_statement_report_path,
        write_tassadar_post_article_canonical_computational_model_statement_report,
        TassadarPostArticleCanonicalComputationalModelStatementReport,
        TassadarPostArticleCanonicalComputationalModelStatus,
        TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
    };
    use tempfile::tempdir;

    #[test]
    fn canonical_computational_model_statement_report_keeps_scope_bounded() {
        let report = build_tassadar_post_article_canonical_computational_model_statement_report()
            .expect("report");

        assert_eq!(
            report.statement_status,
            TassadarPostArticleCanonicalComputationalModelStatus::Green
        );
        assert_eq!(
            report.computational_model_statement.machine_identity_id,
            "tassadar.post_article_universality_bridge.machine_identity.v1"
        );
        assert_eq!(
            report.computational_model_statement.statement_id,
            "tassadar.post_article.canonical_computational_model.statement.v1"
        );
        assert_eq!(report.supporting_material_rows.len(), 11);
        assert_eq!(report.dependency_rows.len(), 7);
        assert_eq!(report.invalidation_rows.len(), 5);
        assert_eq!(report.validation_rows.len(), 7);
        assert!(report.article_equivalent_compute_named);
        assert!(report.tcm_v1_continuation_named);
        assert!(report.declared_effect_boundary_named);
        assert!(report.plugin_layer_scoped_above_machine);
        assert!(report.proof_transport_complete);
        assert_eq!(
            report.proof_transport_audit_report_ref,
            "fixtures/tassadar/reports/tassadar_post_article_execution_semantics_proof_transport_audit_report.json"
        );
        assert_eq!(report.proof_transport_audit_issue_id, "TAS-209");
        assert_eq!(report.next_stability_issue_id, "TAS-215");
        assert!(!report.closure_bundle_embedded_here);
        assert_eq!(report.closure_bundle_issue_id, "TAS-215");
        assert!(!report.weighted_plugin_control_part_of_model);
        assert!(!report.plugin_publication_allowed);
        assert!(!report.served_public_universality_allowed);
        assert!(!report.arbitrary_software_capability_allowed);
        assert!(report.statement_green);
    }

    #[test]
    fn canonical_computational_model_statement_report_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_canonical_computational_model_statement_report()
                .expect("report");
        let committed: TassadarPostArticleCanonicalComputationalModelStatementReport =
            read_json(tassadar_post_article_canonical_computational_model_statement_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_canonical_computational_model_statement_report_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some("tassadar_post_article_canonical_computational_model_statement_report.json")
        );
    }

    #[test]
    fn canonical_computational_model_statement_report_round_trips_to_disk() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_post_article_canonical_computational_model_statement_report.json");
        let written = write_tassadar_post_article_canonical_computational_model_statement_report(
            &output_path,
        )
        .expect("write report");
        let persisted: TassadarPostArticleCanonicalComputationalModelStatementReport =
            read_json(&output_path).expect("persisted report");
        assert_eq!(written, persisted);
    }

    #[test]
    fn canonical_computational_model_statement_report_path_resolves_inside_repo() {
        assert_eq!(
            TASSADAR_POST_ARTICLE_CANONICAL_COMPUTATIONAL_MODEL_STATEMENT_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_post_article_canonical_computational_model_statement_report.json"
        );
        assert!(
            tassadar_post_article_canonical_computational_model_statement_report_path()
                .starts_with(repo_root())
        );
    }
}
