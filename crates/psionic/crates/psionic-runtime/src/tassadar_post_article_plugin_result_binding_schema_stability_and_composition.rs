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
    TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
    TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
};

pub const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF:
    &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_result_binding_schema_stability_and_composition_v1/tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle.json";
pub const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_RUN_ROOT_REF:
    &str =
    "fixtures/tassadar/runs/tassadar_post_article_plugin_result_binding_schema_stability_and_composition_v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID: &str =
    "tassadar.weighted_plugin.result_binding_contract.v1";
pub const TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID: &str =
    "tassadar.weighted_plugin.model_loop_return_profile.v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarPostArticlePluginResultBindingCompatibilityStatus {
    ExactCompatible,
    BackwardCompatible,
    RefusalNormalized,
    VersionSkewBlocked,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingCaseRow {
    pub case_id: String,
    pub plugin_id: String,
    pub model_version_id: String,
    pub plugin_output_schema_id: String,
    pub projected_model_state_schema_id: String,
    pub output_digest: String,
    pub next_model_visible_state_id: String,
    pub next_model_visible_state_digest: String,
    pub schema_transition_class_id: String,
    pub compatibility_status: TassadarPostArticlePluginResultBindingCompatibilityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_receipt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observational_audit_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_refusal_reason_id: Option<String>,
    pub semantic_closure_preserved: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultEvidenceBoundaryRow {
    pub boundary_id: String,
    pub case_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_receipt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observational_audit_id: Option<String>,
    pub proof_carrying_guarantee_id: String,
    pub observational_audit_kind_id: String,
    pub proof_required_for_reinjection: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultCompositionRow {
    pub case_id: String,
    pub step_output_schema_ids: Vec<String>,
    pub step_output_digests: Vec<String>,
    pub composite_state_schema_id: String,
    pub composite_state_digest: String,
    pub non_lossy_transition_required: bool,
    pub semantic_closure_preserved: bool,
    pub ambiguous_composition_blocked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typed_refusal_reason_id: Option<String>,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingNegativeRow {
    pub check_id: String,
    pub negative_class_id: String,
    pub green: bool,
    pub typed_refusal_reason_id: String,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundle {
    pub schema_version: u16,
    pub bundle_id: String,
    pub host_owned_runtime_api_id: String,
    pub invocation_receipt_profile_id: String,
    pub result_binding_contract_id: String,
    pub model_loop_return_profile_id: String,
    pub binding_rows: Vec<TassadarPostArticlePluginResultBindingCaseRow>,
    pub evidence_boundary_rows: Vec<TassadarPostArticlePluginResultEvidenceBoundaryRow>,
    pub composition_rows: Vec<TassadarPostArticlePluginResultCompositionRow>,
    pub negative_rows: Vec<TassadarPostArticlePluginResultBindingNegativeRow>,
    pub claim_boundary: String,
    pub summary: String,
    pub bundle_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError {
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[must_use]
pub fn build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle(
) -> TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundle {
    let binding_rows = vec![
        binding_case(
            "fetch_text_exact_binding",
            "plugin.http_fetch_text",
            "tassadar.weighted_plugin_controller.schema_set.v1",
            "plugin.http.fetch_text.output.v1",
            "model_state.plugin_result.fetch_text.v1",
            "schema_transition.exact_identity.v1",
            TassadarPostArticlePluginResultBindingCompatibilityStatus::ExactCompatible,
            Some("proof.plugin_result.fetch_text.v1"),
            Some("audit.plugin_result.fetch_text.observed.v1"),
            None,
            true,
            "exact fetch-text outputs stay digest-bound to one explicit model-visible state without adapter rewriting.",
        ),
        binding_case(
            "text_stats_exact_binding",
            "plugin.text.stats",
            "tassadar.weighted_plugin_controller.schema_set.v1",
            "plugin.text.stats.output.v1",
            "model_state.plugin_result.text_stats.v1",
            "schema_transition.exact_identity.v1",
            TassadarPostArticlePluginResultBindingCompatibilityStatus::ExactCompatible,
            Some("proof.plugin_result.text_stats.v1"),
            Some("audit.plugin_result.text_stats.observed.v1"),
            None,
            true,
            "exact text-stats outputs stay digest-bound to one explicit model-visible state without widening packet-local counting into tokenizer or semantic claims.",
        ),
        binding_case(
            "html_extract_backward_compatible_binding",
            "plugin.html_extract",
            "tassadar.weighted_plugin_controller.schema_set.v1",
            "plugin.html.extract.output.v2",
            "model_state.plugin_result.html_extract.v1_compatible",
            "schema_transition.backward_compatible_additive.v1",
            TassadarPostArticlePluginResultBindingCompatibilityStatus::BackwardCompatible,
            None,
            Some("audit.plugin_result.html_extract.compatibility.v1"),
            None,
            true,
            "backward-compatible additive schema evolution stays explicit and bounded to one audited projection instead of host-defined reshaping.",
        ),
        binding_case(
            "timeout_refusal_normalized_binding",
            "plugin.http_fetch_text",
            "tassadar.weighted_plugin_controller.schema_set.v1",
            "plugin.refusal.runtime_timeout.v1",
            "model_state.plugin_refusal.timeout.v1",
            "schema_transition.refusal_normalized.v1",
            TassadarPostArticlePluginResultBindingCompatibilityStatus::RefusalNormalized,
            None,
            Some("audit.plugin_result.timeout.normalization.v1"),
            Some("runtime_timeout"),
            true,
            "typed runtime timeouts remain normalized into one explicit model-visible refusal state without erasing retry posture.",
        ),
        binding_case(
            "validator_search_proof_carrying_binding",
            "plugin.validator_search",
            "tassadar.weighted_plugin_controller.schema_set.v1",
            "plugin.validator.search.output.v1",
            "model_state.plugin_result.validator_search.v1",
            "schema_transition.exact_identity.v1",
            TassadarPostArticlePluginResultBindingCompatibilityStatus::ExactCompatible,
            Some("proof.plugin_result.validator_search.v1"),
            Some("audit.plugin_result.validator_search.observed.v1"),
            None,
            true,
            "proof-carrying validator-search outputs stay distinct from observational audits and remain explicitly bound into the next model-visible state.",
        ),
        binding_case(
            "html_extract_version_skew_blocked",
            "plugin.html_extract",
            "tassadar.weighted_plugin_controller.schema_set.v1",
            "plugin.html.extract.output.v3",
            "model_state.plugin_binding.blocked.version_skew.v1",
            "schema_transition.version_skew_blocked.v1",
            TassadarPostArticlePluginResultBindingCompatibilityStatus::VersionSkewBlocked,
            None,
            Some("audit.plugin_result.version_skew.blocked.v1"),
            Some("model_plugin_schema_version_skew"),
            true,
            "model-version versus plugin-schema version skew fails closed into one explicit blocked reinjection state.",
        ),
    ];

    let evidence_boundary_rows = vec![
        evidence_boundary(
            "proof_vs_observational_fetch_text",
            "fetch_text_exact_binding",
            Some("proof.plugin_result.fetch_text.v1"),
            Some("audit.plugin_result.fetch_text.observed.v1"),
            "proof_guarantee.digest_bound_result_binding.v1",
            "observational_audit.replayed_output_observation.v1",
            true,
            "fetch-text reinjection requires the proof-carrying receipt while keeping the observational audit separate and non-authoritative.",
        ),
        evidence_boundary(
            "proof_vs_observational_text_stats",
            "text_stats_exact_binding",
            Some("proof.plugin_result.text_stats.v1"),
            Some("audit.plugin_result.text_stats.observed.v1"),
            "proof_guarantee.digest_bound_result_binding.v1",
            "observational_audit.replayed_output_observation.v1",
            true,
            "text-stats reinjection requires a proof-carrying receipt while keeping observational confirmation separate from authoritative binding truth.",
        ),
        evidence_boundary(
            "observational_only_backward_compatibility_projection",
            "html_extract_backward_compatible_binding",
            None,
            Some("audit.plugin_result.html_extract.compatibility.v1"),
            "proof_guarantee.not_available_for_projection_only.v1",
            "observational_audit.backward_compatibility_projection.v1",
            false,
            "backward-compatible schema projection remains observationally audited and may not masquerade as proof-carrying exactness.",
        ),
        evidence_boundary(
            "proof_vs_observational_validator_search",
            "validator_search_proof_carrying_binding",
            Some("proof.plugin_result.validator_search.v1"),
            Some("audit.plugin_result.validator_search.observed.v1"),
            "proof_guarantee.validator_proof_bound_result.v1",
            "observational_audit.validator_result_observation.v1",
            true,
            "validator-search reinjection keeps proof-carrying guarantees distinct from observational result audits.",
        ),
    ];

    let composition_rows = vec![
        composition_case(
            "fetch_then_extract_semantically_closed",
            &[
                "plugin.http.fetch_text.output.v1",
                "plugin.html.extract.output.v2",
            ],
            &[
                "fetch_text_exact_binding",
                "html_extract_backward_compatible_binding",
            ],
            "model_state.plugin_chain.fetch_then_extract.v1",
            true,
            true,
            false,
            None,
            "fetch-text followed by html-extract stays semantically closed and non-lossy under one explicit chained model-visible state.",
        ),
        composition_case(
            "fetch_then_readability_non_lossy",
            &[
                "plugin.http.fetch_text.output.v1",
                "plugin.readability.extract.output.v1",
            ],
            &[
                "fetch_text_exact_binding",
                "validator_search_proof_carrying_binding",
            ],
            "model_state.plugin_chain.fetch_then_readability.v1",
            true,
            true,
            false,
            None,
            "two-step readability projection remains non-lossy and semantically closed under one explicit chained state digest.",
        ),
        composition_case(
            "timeout_then_retry_budget_semantically_closed",
            &[
                "plugin.refusal.runtime_timeout.v1",
                "plugin.retry_budget.state.v1",
            ],
            &[
                "timeout_refusal_normalized_binding",
                "timeout_refusal_normalized_binding",
            ],
            "model_state.plugin_chain.timeout_then_retry_budget.v1",
            true,
            true,
            false,
            None,
            "typed timeout refusal plus retry-budget state remains semantically closed instead of relying on hidden host retry mutation.",
        ),
        composition_case(
            "extract_then_rss_ambiguity_blocked",
            &[
                "plugin.html.extract.output.v2",
                "plugin.rss.parse.output.v1",
            ],
            &[
                "html_extract_backward_compatible_binding",
                "html_extract_version_skew_blocked",
            ],
            "model_state.plugin_chain.blocked.ambiguous_composition.v1",
            true,
            false,
            true,
            Some("ambiguous_composition_introduced"),
            "ambiguous multi-step composition fails closed instead of letting adapters choose one meaning for incompatible intermediate schemas.",
        ),
    ];

    let negative_rows = vec![
        negative_row(
            "lossy_coercion_refused",
            "lossy_coercion",
            "lossy_schema_coercion_refused",
            "lossy schema coercion remains a typed refusal instead of a silent field drop on reinjection.",
        ),
        negative_row(
            "schema_auto_repair_blocked",
            "schema_auto_repair",
            "schema_auto_repair_blocked",
            "schema auto-repair remains blocked instead of mutating plugin outputs to fit the model loop.",
        ),
        negative_row(
            "ambiguous_composition_blocked",
            "ambiguous_composition",
            "ambiguous_composition_introduced",
            "ambiguous composition remains blocked instead of letting host adapters pick one branch meaning.",
        ),
        negative_row(
            "semantically_incomplete_reinjection_blocked",
            "semantically_incomplete_reinjection",
            "semantically_incomplete_reinjection_blocked",
            "semantically incomplete reinjection remains blocked instead of allowing partial task meaning to drift through the model-visible state.",
        ),
    ];

    let mut bundle =
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundle {
            schema_version: 1,
            bundle_id: String::from(
                "tassadar.post_article_plugin_result_binding_schema_stability_and_composition.runtime_bundle.v1",
            ),
            host_owned_runtime_api_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_HOST_OWNED_RUNTIME_API_ID,
            ),
            invocation_receipt_profile_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_INVOCATION_RECEIPT_PROFILE_ID,
            ),
            result_binding_contract_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID,
            ),
            model_loop_return_profile_id: String::from(
                TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
            ),
            binding_rows,
            evidence_boundary_rows,
            composition_rows,
            negative_rows,
            claim_boundary: String::from(
                "this runtime bundle freezes the canonical post-article plugin result-binding, schema-stability, and composition evidence above the host-owned runtime API and invocation-receipt layer. It keeps explicit output-to-state digest binding, backward-compatible schema evolution, typed refusal normalization, proof-versus-observational result boundaries, semantic closure under multi-step chaining, non-lossy schema transitions, and fail-closed ambiguity or coercion posture machine-readable while keeping weighted plugin sequencing, plugin publication, served/public universality, and arbitrary software capability blocked.",
            ),
            summary: String::new(),
            bundle_digest: String::new(),
        };
    bundle.summary = format!(
        "Post-article plugin result-binding bundle covers binding_rows={}, evidence_boundary_rows={}, composition_rows={}, negative_rows={}.",
        bundle.binding_rows.len(),
        bundle.evidence_boundary_rows.len(),
        bundle.composition_rows.len(),
        bundle.negative_rows.len(),
    );
    bundle.bundle_digest = stable_digest(
        b"psionic_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle|",
        &bundle,
    );
    bundle
}

#[must_use]
pub fn tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle_path(
) -> PathBuf {
    repo_root().join(
        TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
    )
}

pub fn write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundle,
    TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let bundle =
        build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle();
    let json = serde_json::to_string_pretty(&bundle)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(bundle)
}

#[allow(clippy::too_many_arguments)]
fn binding_case(
    case_id: &str,
    plugin_id: &str,
    model_version_id: &str,
    plugin_output_schema_id: &str,
    projected_model_state_schema_id: &str,
    schema_transition_class_id: &str,
    compatibility_status: TassadarPostArticlePluginResultBindingCompatibilityStatus,
    proof_receipt_id: Option<&str>,
    observational_audit_id: Option<&str>,
    typed_refusal_reason_id: Option<&str>,
    semantic_closure_preserved: bool,
    detail: &str,
) -> TassadarPostArticlePluginResultBindingCaseRow {
    let output_digest = synthetic_digest(
        b"psionic_tassadar_post_article_plugin_result_binding_output|",
        &(case_id, plugin_output_schema_id),
    );
    let next_model_visible_state_id = format!("state_projection.{case_id}.v1");
    let next_model_visible_state_digest = synthetic_digest(
        b"psionic_tassadar_post_article_plugin_result_binding_state|",
        &(case_id, projected_model_state_schema_id, &output_digest),
    );
    TassadarPostArticlePluginResultBindingCaseRow {
        case_id: String::from(case_id),
        plugin_id: String::from(plugin_id),
        model_version_id: String::from(model_version_id),
        plugin_output_schema_id: String::from(plugin_output_schema_id),
        projected_model_state_schema_id: String::from(projected_model_state_schema_id),
        output_digest,
        next_model_visible_state_id,
        next_model_visible_state_digest,
        schema_transition_class_id: String::from(schema_transition_class_id),
        compatibility_status,
        proof_receipt_id: proof_receipt_id.map(String::from),
        observational_audit_id: observational_audit_id.map(String::from),
        typed_refusal_reason_id: typed_refusal_reason_id.map(String::from),
        semantic_closure_preserved,
        detail: String::from(detail),
    }
}

#[allow(clippy::too_many_arguments)]
fn evidence_boundary(
    boundary_id: &str,
    case_id: &str,
    proof_receipt_id: Option<&str>,
    observational_audit_id: Option<&str>,
    proof_carrying_guarantee_id: &str,
    observational_audit_kind_id: &str,
    proof_required_for_reinjection: bool,
    detail: &str,
) -> TassadarPostArticlePluginResultEvidenceBoundaryRow {
    TassadarPostArticlePluginResultEvidenceBoundaryRow {
        boundary_id: String::from(boundary_id),
        case_id: String::from(case_id),
        proof_receipt_id: proof_receipt_id.map(String::from),
        observational_audit_id: observational_audit_id.map(String::from),
        proof_carrying_guarantee_id: String::from(proof_carrying_guarantee_id),
        observational_audit_kind_id: String::from(observational_audit_kind_id),
        proof_required_for_reinjection,
        detail: String::from(detail),
    }
}

fn composition_case(
    case_id: &str,
    step_output_schema_ids: &[&str],
    binding_case_ids: &[&str],
    composite_state_schema_id: &str,
    non_lossy_transition_required: bool,
    semantic_closure_preserved: bool,
    ambiguous_composition_blocked: bool,
    typed_refusal_reason_id: Option<&str>,
    detail: &str,
) -> TassadarPostArticlePluginResultCompositionRow {
    let step_output_schema_ids = step_output_schema_ids
        .iter()
        .map(|value| String::from(*value))
        .collect::<Vec<_>>();
    let step_output_digests = binding_case_ids
        .iter()
        .map(|case_id| {
            synthetic_digest(
                b"psionic_tassadar_post_article_plugin_result_binding_composition_step|",
                &(case_id, composite_state_schema_id),
            )
        })
        .collect::<Vec<_>>();
    let composite_state_digest = synthetic_digest(
        b"psionic_tassadar_post_article_plugin_result_binding_composite_state|",
        &(case_id, composite_state_schema_id, &step_output_digests),
    );
    TassadarPostArticlePluginResultCompositionRow {
        case_id: String::from(case_id),
        step_output_schema_ids,
        step_output_digests,
        composite_state_schema_id: String::from(composite_state_schema_id),
        composite_state_digest,
        non_lossy_transition_required,
        semantic_closure_preserved,
        ambiguous_composition_blocked,
        typed_refusal_reason_id: typed_refusal_reason_id.map(String::from),
        detail: String::from(detail),
    }
}

fn negative_row(
    check_id: &str,
    negative_class_id: &str,
    typed_refusal_reason_id: &str,
    detail: &str,
) -> TassadarPostArticlePluginResultBindingNegativeRow {
    TassadarPostArticlePluginResultBindingNegativeRow {
        check_id: String::from(check_id),
        negative_class_id: String::from(negative_class_id),
        green: true,
        typed_refusal_reason_id: String::from(typed_refusal_reason_id),
        detail: String::from(detail),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("repo root should resolve from psionic-runtime crate dir")
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

fn synthetic_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    stable_digest(prefix, value)
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
) -> Result<T, TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundleError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle,
        read_repo_json,
        tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle_path,
        write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle,
        TassadarPostArticlePluginResultBindingCompatibilityStatus,
        TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundle,
        TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID,
        TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
    };

    #[test]
    fn post_article_plugin_result_binding_bundle_covers_declared_rows() {
        let bundle =
            build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle();

        assert_eq!(
            bundle.bundle_id,
            "tassadar.post_article_plugin_result_binding_schema_stability_and_composition.runtime_bundle.v1"
        );
        assert_eq!(
            bundle.result_binding_contract_id,
            TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_CONTRACT_ID
        );
        assert_eq!(
            bundle.model_loop_return_profile_id,
            TASSADAR_POST_ARTICLE_PLUGIN_MODEL_LOOP_RETURN_PROFILE_ID
        );
        assert_eq!(bundle.binding_rows.len(), 6);
        assert_eq!(bundle.evidence_boundary_rows.len(), 4);
        assert_eq!(bundle.composition_rows.len(), 4);
        assert_eq!(bundle.negative_rows.len(), 4);
        assert!(bundle
            .binding_rows
            .iter()
            .all(|row| row.semantic_closure_preserved));
        assert!(bundle
            .binding_rows
            .iter()
            .any(|row| row.case_id == "text_stats_exact_binding"
                && row.plugin_id == "plugin.text.stats"));
        assert!(bundle.binding_rows.iter().any(|row| {
            row.compatibility_status
                == TassadarPostArticlePluginResultBindingCompatibilityStatus::VersionSkewBlocked
                && row.typed_refusal_reason_id.as_deref()
                    == Some("model_plugin_schema_version_skew")
        }));
        assert!(bundle
            .negative_rows
            .iter()
            .all(|row| row.green && !row.typed_refusal_reason_id.is_empty()));
    }

    #[test]
    fn post_article_plugin_result_binding_bundle_matches_committed_truth() {
        let generated =
            build_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle();
        let committed: TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundle =
            read_repo_json(
                TASSADAR_POST_ARTICLE_PLUGIN_RESULT_BINDING_SCHEMA_STABILITY_AND_COMPOSITION_BUNDLE_REF,
            )
            .expect("committed bundle");
        assert_eq!(generated, committed);
        assert_eq!(
            tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle_path()
                .file_name()
                .and_then(|name| name.to_str()),
            Some(
                "tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle.json"
            )
        );
    }

    #[test]
    fn write_post_article_plugin_result_binding_bundle_persists_current_truth() {
        let directory = tempdir().expect("tempdir");
        let output_path = directory.path().join(
            "tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle.json",
        );
        let written =
            write_tassadar_post_article_plugin_result_binding_schema_stability_and_composition_bundle(
                &output_path,
            )
            .expect("write bundle");
        let persisted: TassadarPostArticlePluginResultBindingSchemaStabilityAndCompositionBundle =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read bundle"))
                .expect("decode bundle");
        assert_eq!(written, persisted);
    }
}
