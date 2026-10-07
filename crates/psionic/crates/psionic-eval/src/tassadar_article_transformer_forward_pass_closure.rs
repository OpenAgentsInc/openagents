use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_models::TassadarArticleTransformer;
use psionic_runtime::{
    RuntimeManifestArtifactKind, TassadarArticleTransformerForwardPassEvidenceBundle,
    TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLAIMS_PROFILE_ID,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    read_tassadar_article_transformer_forward_pass_evidence_bundle,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TassadarArticleTransformerForwardPassEvidenceError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_EVIDENCE_BUNDLE_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_forward_pass_closure_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-165";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerForwardPassValidationKind {
    ModelArtifactIdentity,
    RuntimeManifestBindings,
    ForwardPassTraceHooks,
    DeterministicReplay,
    DecodeReceipt,
    CheckpointLineage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerForwardPassCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarArticleTransformerForwardPassValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerForwardPassBoundaryReview {
    pub model_module_ref: String,
    pub transformer_module_ref: String,
    pub runtime_module_ref: String,
    pub model_defines_forward_with_runtime_evidence: bool,
    pub model_binds_runtime_evidence_builder: bool,
    pub transformer_defines_encoder_decoder_forward_output: bool,
    pub runtime_defines_forward_pass_evidence_bundle: bool,
    pub runtime_binds_runtime_manifest: bool,
    pub runtime_binds_execution_proof_bundle: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerForwardPassAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerForwardPassClosureReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleTransformerForwardPassAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub evidence_bundle_ref: String,
    pub evidence_bundle: TassadarArticleTransformerForwardPassEvidenceBundle,
    pub case_rows: Vec<TassadarArticleTransformerForwardPassCaseRow>,
    pub boundary_review: TassadarArticleTransformerForwardPassBoundaryReview,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub article_transformer_forward_pass_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerForwardPassClosureReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    CanonicalBoundary(#[from] TassadarCanonicalTransformerStackBoundaryReportError),
    #[error(transparent)]
    ForwardPassEvidence(#[from] TassadarArticleTransformerForwardPassEvidenceError),
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

pub fn build_tassadar_article_transformer_forward_pass_closure_report(
) -> Result<
    TassadarArticleTransformerForwardPassClosureReport,
    TassadarArticleTransformerForwardPassClosureReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let canonical_boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let evidence_bundle = read_tassadar_article_transformer_forward_pass_evidence_bundle(
        TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_EVIDENCE_BUNDLE_REF,
    )?;
    let case_rows = case_rows(&evidence_bundle);
    let boundary_review = boundary_review(&evidence_bundle)?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        canonical_boundary_report,
        evidence_bundle,
        case_rows,
        boundary_review,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    evidence_bundle: TassadarArticleTransformerForwardPassEvidenceBundle,
    case_rows: Vec<TassadarArticleTransformerForwardPassCaseRow>,
    boundary_review: TassadarArticleTransformerForwardPassBoundaryReview,
) -> TassadarArticleTransformerForwardPassClosureReport {
    let acceptance_gate_tie = TassadarArticleTransformerForwardPassAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let all_required_cases_present = case_rows
        .iter()
        .map(|row| row.validation_kind)
        .collect::<BTreeSet<_>>()
        == required_validation_kinds();
    let all_cases_pass = case_rows.iter().all(|row| row.passed);
    let article_transformer_forward_pass_contract_green = acceptance_gate_tie
        .tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && all_required_cases_present
        && all_cases_pass
        && boundary_review.passed;
    let article_equivalence_green = article_transformer_forward_pass_contract_green
        && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTransformerForwardPassClosureReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_transformer_forward_pass_closure.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        evidence_bundle_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_EVIDENCE_BUNDLE_REF,
        ),
        evidence_bundle,
        case_rows,
        boundary_review,
        all_required_cases_present,
        all_cases_pass,
        article_transformer_forward_pass_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report freezes the runtime evidence closure for one canonical article-Transformer forward-pass lane only. It proves that model identity, run config, attention-trace summaries, deterministic replay, decode receipts, and checkpoint lineage now bind into the Psionic runtime-manifest and proof-bundle substrate on top of the owned `psionic-transformer` route. It does not claim final article trace-vocabulary closure, weight-production closure, reference-linear exactness proof, fast-route promotion, benchmark parity, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article Transformer forward-pass closure now records case_rows={}, all_required_cases_present={}, all_cases_pass={}, boundary_review_passed={}, tied_requirement_satisfied={}, article_transformer_forward_pass_contract_green={}, and article_equivalence_green={}.",
        report.case_rows.len(),
        report.all_required_cases_present,
        report.all_cases_pass,
        report.boundary_review.passed,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.article_transformer_forward_pass_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_forward_pass_closure_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarArticleTransformerForwardPassValidationKind> {
    BTreeSet::from([
        TassadarArticleTransformerForwardPassValidationKind::ModelArtifactIdentity,
        TassadarArticleTransformerForwardPassValidationKind::RuntimeManifestBindings,
        TassadarArticleTransformerForwardPassValidationKind::ForwardPassTraceHooks,
        TassadarArticleTransformerForwardPassValidationKind::DeterministicReplay,
        TassadarArticleTransformerForwardPassValidationKind::DecodeReceipt,
        TassadarArticleTransformerForwardPassValidationKind::CheckpointLineage,
    ])
}

fn case_rows(
    evidence_bundle: &TassadarArticleTransformerForwardPassEvidenceBundle,
) -> Vec<TassadarArticleTransformerForwardPassCaseRow> {
    vec![
        model_artifact_identity_case(evidence_bundle),
        runtime_manifest_bindings_case(evidence_bundle),
        forward_pass_trace_hooks_case(evidence_bundle),
        deterministic_replay_case(evidence_bundle),
        decode_receipt_case(evidence_bundle),
        checkpoint_lineage_case(evidence_bundle),
    ]
}

fn model_artifact_identity_case(
    evidence_bundle: &TassadarArticleTransformerForwardPassEvidenceBundle,
) -> TassadarArticleTransformerForwardPassCaseRow {
    let model_descriptor_binding = evidence_bundle
        .runtime_manifest
        .artifact_bindings
        .iter()
        .find(|binding| binding.kind == RuntimeManifestArtifactKind::ModelDescriptor);
    let policy_weights_binding = evidence_bundle
        .runtime_manifest
        .artifact_bindings
        .iter()
        .find(|binding| binding.kind == RuntimeManifestArtifactKind::PolicyWeights);
    let passed = !evidence_bundle.model_artifact.model_id.trim().is_empty()
        && !evidence_bundle.model_artifact.descriptor_digest.trim().is_empty()
        && !evidence_bundle.model_artifact.weight_bundle_digest.trim().is_empty()
        && model_descriptor_binding
            .map(|binding| binding.digest.as_str())
            == Some(evidence_bundle.model_artifact.descriptor_digest.as_str())
        && policy_weights_binding
            .map(|binding| binding.digest.as_str())
            == Some(evidence_bundle.model_artifact.weight_bundle_digest.as_str());
    case_row(
        "model_artifact_identity",
        TassadarArticleTransformerForwardPassValidationKind::ModelArtifactIdentity,
        passed,
        format!(
            "model_id={} descriptor_digest={} weight_bundle_digest={}",
            evidence_bundle.model_artifact.model_id,
            evidence_bundle.model_artifact.descriptor_digest,
            evidence_bundle.model_artifact.weight_bundle_digest,
        ),
    )
}

fn runtime_manifest_bindings_case(
    evidence_bundle: &TassadarArticleTransformerForwardPassEvidenceBundle,
) -> TassadarArticleTransformerForwardPassCaseRow {
    let claims_profile_matches = evidence_bundle.runtime_manifest.claims_profile_id.as_deref()
        == Some(TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLAIMS_PROFILE_ID);
    let trace_binding_matches = evidence_bundle
        .runtime_manifest
        .artifact_bindings
        .iter()
        .find(|binding| binding.kind == RuntimeManifestArtifactKind::ExecutionTrace)
        .map(|binding| binding.digest.as_str())
        == Some(evidence_bundle.trace_artifact.artifact_digest.as_str());
    let residency = evidence_bundle.proof_bundle.artifact_residency.as_ref();
    let residency_matches = residency
        .map(|artifact_residency| {
            artifact_residency.weight_bundle_digest.as_deref()
                == Some(evidence_bundle.model_artifact.weight_bundle_digest.as_str())
                && artifact_residency
                    .output_artifact_digests
                    .contains(&evidence_bundle.trace_artifact.artifact_digest)
                && artifact_residency
                    .output_artifact_digests
                    .contains(&evidence_bundle.decode_receipt.receipt_digest)
                && artifact_residency
                    .output_artifact_digests
                    .contains(&evidence_bundle.replay_receipt.receipt_digest)
        })
        .unwrap_or(false);
    let passed = claims_profile_matches
        && trace_binding_matches
        && !evidence_bundle.runtime_manifest.identity_digest.trim().is_empty()
        && !evidence_bundle.runtime_manifest.manifest_digest.trim().is_empty()
        && residency_matches;
    case_row(
        "runtime_manifest_bindings",
        TassadarArticleTransformerForwardPassValidationKind::RuntimeManifestBindings,
        passed,
        format!(
            "claims_profile_matches={} trace_binding_matches={} runtime_manifest_identity_digest={} proof_bundle_status={:?}",
            claims_profile_matches,
            trace_binding_matches,
            evidence_bundle.runtime_manifest.identity_digest,
            evidence_bundle.proof_bundle.status,
        ),
    )
}

fn forward_pass_trace_hooks_case(
    evidence_bundle: &TassadarArticleTransformerForwardPassEvidenceBundle,
) -> TassadarArticleTransformerForwardPassCaseRow {
    let reference_model =
        TassadarArticleTransformer::canonical_reference().expect("canonical article Transformer");
    let config = &reference_model.descriptor().config;
    let passed = evidence_bundle.trace_artifact.model_module_ref == evidence_bundle.model_module_ref
        && evidence_bundle.trace_artifact.transformer_module_ref
            == evidence_bundle.transformer_module_ref
        && evidence_bundle.trace_artifact.encoder_layer_traces.len() == config.encoder_layer_count
        && evidence_bundle.trace_artifact.decoder_self_attention_traces.len()
            == config.decoder_layer_count
        && evidence_bundle.trace_artifact.decoder_cross_attention_traces.len()
            == config.decoder_layer_count
        && evidence_bundle
            .trace_artifact
            .encoder_layer_traces
            .iter()
            .all(|trace| trace.channel_kind == "encoder_self_attention" && trace.element_count > 0)
        && evidence_bundle
            .trace_artifact
            .decoder_self_attention_traces
            .iter()
            .all(|trace| trace.channel_kind == "decoder_self_attention" && trace.element_count > 0)
        && evidence_bundle
            .trace_artifact
            .decoder_cross_attention_traces
            .iter()
            .all(|trace| trace.channel_kind == "decoder_cross_attention" && trace.element_count > 0);
    case_row(
        "forward_pass_trace_hooks",
        TassadarArticleTransformerForwardPassValidationKind::ForwardPassTraceHooks,
        passed,
        format!(
            "encoder_layer_traces={} decoder_self_attention_traces={} decoder_cross_attention_traces={}",
            evidence_bundle.trace_artifact.encoder_layer_traces.len(),
            evidence_bundle.trace_artifact.decoder_self_attention_traces.len(),
            evidence_bundle.trace_artifact.decoder_cross_attention_traces.len(),
        ),
    )
}

fn deterministic_replay_case(
    evidence_bundle: &TassadarArticleTransformerForwardPassEvidenceBundle,
) -> TassadarArticleTransformerForwardPassCaseRow {
    let replay = &evidence_bundle.replay_receipt;
    let passed = replay.deterministic_match
        && replay.expected_forward_output_digest == replay.replayed_forward_output_digest
        && replay.expected_trace_digest == replay.replayed_trace_digest
        && evidence_bundle.proof_bundle.failure_reason.is_none();
    case_row(
        "deterministic_replay",
        TassadarArticleTransformerForwardPassValidationKind::DeterministicReplay,
        passed,
        format!(
            "deterministic_match={} expected_forward_output_digest={} replayed_forward_output_digest={}",
            replay.deterministic_match,
            replay.expected_forward_output_digest,
            replay.replayed_forward_output_digest,
        ),
    )
}

fn decode_receipt_case(
    evidence_bundle: &TassadarArticleTransformerForwardPassEvidenceBundle,
) -> TassadarArticleTransformerForwardPassCaseRow {
    let decode = &evidence_bundle.decode_receipt;
    let passed = decode.decode_strategy == "greedy_argmax"
        && !decode.predicted_token_ids.is_empty()
        && decode.predicted_token_ids == evidence_bundle.trace_artifact.predicted_token_ids
        && !decode.decoded_token_digest.trim().is_empty();
    case_row(
        "decode_receipt",
        TassadarArticleTransformerForwardPassValidationKind::DecodeReceipt,
        passed,
        format!(
            "decode_strategy={} decoded_token_count={} decoded_token_digest={}",
            decode.decode_strategy,
            decode.predicted_token_ids.len(),
            decode.decoded_token_digest,
        ),
    )
}

fn checkpoint_lineage_case(
    evidence_bundle: &TassadarArticleTransformerForwardPassEvidenceBundle,
) -> TassadarArticleTransformerForwardPassCaseRow {
    let checkpoint_lineage = evidence_bundle.checkpoint_lineage.as_ref();
    let checkpoint_binding = evidence_bundle
        .runtime_manifest
        .artifact_bindings
        .iter()
        .find(|binding| binding.kind == RuntimeManifestArtifactKind::Checkpoint);
    let checkpoint_manifest_binding = evidence_bundle
        .runtime_manifest
        .static_config_bindings
        .iter()
        .find(|binding| binding.key == "tassadar.article_transformer.checkpoint_manifest_digest");
    let passed = checkpoint_lineage
        .map(|lineage| {
            !lineage.checkpoint.object_digest.trim().is_empty()
                && lineage.parent_checkpoint_ref.is_some()
                && lineage.parent_manifest_digest.is_some()
                && checkpoint_binding.map(|binding| binding.digest.as_str())
                    == Some(lineage.checkpoint.object_digest.as_str())
                && checkpoint_manifest_binding.map(|binding| binding.value_digest.as_str())
                    == Some(lineage.checkpoint.manifest_digest.as_str())
        })
        .unwrap_or(false);
    case_row(
        "checkpoint_lineage",
        TassadarArticleTransformerForwardPassValidationKind::CheckpointLineage,
        passed,
        format!(
            "checkpoint_present={} parent_checkpoint_present={} parent_manifest_present={}",
            checkpoint_lineage.is_some(),
            checkpoint_lineage
                .and_then(|lineage| lineage.parent_checkpoint_ref.as_ref())
                .is_some(),
            checkpoint_lineage
                .and_then(|lineage| lineage.parent_manifest_digest.as_ref())
                .is_some(),
        ),
    )
}

fn boundary_review(
    evidence_bundle: &TassadarArticleTransformerForwardPassEvidenceBundle,
) -> Result<
    TassadarArticleTransformerForwardPassBoundaryReview,
    TassadarArticleTransformerForwardPassClosureReportError,
> {
    let repo_root = repo_root();
    let model_source = fs::read_to_string(repo_root.join(&evidence_bundle.model_module_ref)).map_err(
        |error| TassadarArticleTransformerForwardPassClosureReportError::Read {
            path: repo_root.join(&evidence_bundle.model_module_ref).display().to_string(),
            error,
        },
    )?;
    let transformer_source =
        fs::read_to_string(repo_root.join(&evidence_bundle.transformer_module_ref)).map_err(
            |error| TassadarArticleTransformerForwardPassClosureReportError::Read {
                path: repo_root
                    .join(&evidence_bundle.transformer_module_ref)
                    .display()
                    .to_string(),
                error,
            },
        )?;
    let runtime_source = fs::read_to_string(repo_root.join(&evidence_bundle.runtime_module_ref)).map_err(
        |error| TassadarArticleTransformerForwardPassClosureReportError::Read {
            path: repo_root.join(&evidence_bundle.runtime_module_ref).display().to_string(),
            error,
        },
    )?;
    let review = TassadarArticleTransformerForwardPassBoundaryReview {
        model_module_ref: evidence_bundle.model_module_ref.clone(),
        transformer_module_ref: evidence_bundle.transformer_module_ref.clone(),
        runtime_module_ref: evidence_bundle.runtime_module_ref.clone(),
        model_defines_forward_with_runtime_evidence: model_source
            .contains("forward_with_runtime_evidence"),
        model_binds_runtime_evidence_builder: model_source
            .contains("build_tassadar_article_transformer_forward_pass_evidence_bundle"),
        transformer_defines_encoder_decoder_forward_output: transformer_source
            .contains("EncoderDecoderTransformerForwardOutput"),
        runtime_defines_forward_pass_evidence_bundle: runtime_source
            .contains("TassadarArticleTransformerForwardPassEvidenceBundle"),
        runtime_binds_runtime_manifest: runtime_source.contains("RuntimeManifest::new"),
        runtime_binds_execution_proof_bundle: runtime_source.contains("ExecutionProofBundle::new"),
        passed: false,
        detail: String::new(),
    };
    let mut review = review;
    review.passed = review.model_defines_forward_with_runtime_evidence
        && review.model_binds_runtime_evidence_builder
        && review.transformer_defines_encoder_decoder_forward_output
        && review.runtime_defines_forward_pass_evidence_bundle
        && review.runtime_binds_runtime_manifest
        && review.runtime_binds_execution_proof_bundle;
    review.detail = String::from(
        "the canonical article wrapper emits runtime evidence from `psionic-models`, the reusable encoder-decoder forward output remains owned by `psionic-transformer`, and runtime-manifest plus proof-bundle construction remains owned by `psionic-runtime`.",
    );
    Ok(review)
}

fn case_row(
    case_id: &str,
    validation_kind: TassadarArticleTransformerForwardPassValidationKind,
    passed: bool,
    detail: impl Into<String>,
) -> TassadarArticleTransformerForwardPassCaseRow {
    TassadarArticleTransformerForwardPassCaseRow {
        case_id: String::from(case_id),
        validation_kind,
        passed,
        detail: detail.into(),
    }
}

pub fn tassadar_article_transformer_forward_pass_closure_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLOSURE_REPORT_REF)
}

pub fn write_tassadar_article_transformer_forward_pass_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerForwardPassClosureReport,
    TassadarArticleTransformerForwardPassClosureReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerForwardPassClosureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_forward_pass_closure_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerForwardPassClosureReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
) -> Result<T, TassadarArticleTransformerForwardPassClosureReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleTransformerForwardPassClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerForwardPassClosureReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_forward_pass_closure_report, read_json,
        tassadar_article_transformer_forward_pass_closure_report_path,
        write_tassadar_article_transformer_forward_pass_closure_report,
        TassadarArticleTransformerForwardPassClosureReport,
        TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLOSURE_REPORT_REF,
    };

    #[test]
    fn article_transformer_forward_pass_closure_tracks_green_runtime_lane_without_final_green() {
        let report =
            build_tassadar_article_transformer_forward_pass_closure_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(report.acceptance_gate_tie.acceptance_status, super::TassadarArticleEquivalenceAcceptanceStatus::Green);
        assert!(report.article_transformer_forward_pass_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_rows.len(), 6);
        assert_eq!(report.case_rows.iter().filter(|row| row.passed).count(), 6);
    }

    #[test]
    fn article_transformer_forward_pass_closure_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_forward_pass_closure_report().expect("report");
        let committed: TassadarArticleTransformerForwardPassClosureReport =
            read_json(tassadar_article_transformer_forward_pass_closure_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_forward_pass_closure_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_forward_pass_closure_report.json");
        let written = write_tassadar_article_transformer_forward_pass_closure_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleTransformerForwardPassClosureReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_forward_pass_closure_report_path()
                .strip_prefix(super::repo_root())
                .expect("repo-relative path")
                .to_string_lossy(),
            TASSADAR_ARTICLE_TRANSFORMER_FORWARD_PASS_CLOSURE_REPORT_REF
        );
    }
}
