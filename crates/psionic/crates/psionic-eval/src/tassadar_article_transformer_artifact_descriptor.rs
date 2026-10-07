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

use psionic_models::{
    TassadarArticleTransformer, TassadarArticleTransformerError, WeightFormat, WeightSource,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_ARTIFACT_DESCRIPTOR_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_artifact_descriptor_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-168";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerArtifactValidationKind {
    CanonicalDescriptorArtifactBacked,
    TraceBoundDescriptorArtifactBacked,
    ArtifactRoundtrip,
    TensorInventoryMetadata,
    RuntimeArtifactBinding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerArtifactCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarArticleTransformerArtifactValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerCommittedArtifactReview {
    pub canonical_descriptor_ref: String,
    pub canonical_artifact_ref: String,
    pub trace_bound_descriptor_ref: String,
    pub trace_bound_artifact_ref: String,
    pub all_committed_refs_exist: bool,
    pub all_refs_under_fixtures_tassadar_models: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerArtifactAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerArtifactDescriptorReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleTransformerArtifactAcceptanceGateTie,
    pub committed_artifact_review: TassadarArticleTransformerCommittedArtifactReview,
    pub case_rows: Vec<TassadarArticleTransformerArtifactCaseRow>,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub artifact_descriptor_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerArtifactDescriptorReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
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

pub fn build_tassadar_article_transformer_artifact_descriptor_report() -> Result<
    TassadarArticleTransformerArtifactDescriptorReport,
    TassadarArticleTransformerArtifactDescriptorReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let committed_artifact_review = committed_artifact_review();
    let case_rows = case_rows();
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        committed_artifact_review,
        case_rows,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    committed_artifact_review: TassadarArticleTransformerCommittedArtifactReview,
    case_rows: Vec<TassadarArticleTransformerArtifactCaseRow>,
) -> TassadarArticleTransformerArtifactDescriptorReport {
    let acceptance_gate_tie = TassadarArticleTransformerArtifactAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
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
    let artifact_descriptor_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && committed_artifact_review.all_committed_refs_exist
        && committed_artifact_review.all_refs_under_fixtures_tassadar_models
        && all_required_cases_present
        && all_cases_pass;
    let article_equivalence_green =
        artifact_descriptor_contract_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTransformerArtifactDescriptorReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_transformer_artifact_descriptor.report.v1"),
        acceptance_gate_tie,
        committed_artifact_review,
        case_rows,
        all_required_cases_present,
        all_cases_pass,
        artifact_descriptor_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report freezes the article-Transformer descriptor and weight-artifact closure only. It proves that the canonical and trace-bound article descriptors are now safetensors-backed, non-fixture, and bound to explicit tensor inventory plus runtime-visible artifact identity. It does not claim reference-linear exactness, fast-route closure, benchmark parity, single-run closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article Transformer artifact descriptor closure now records case_rows={}, all_required_cases_present={}, all_cases_pass={}, committed_refs_exist={}, tied_requirement_satisfied={}, artifact_descriptor_contract_green={}, and article_equivalence_green={}.",
        report.case_rows.len(),
        report.all_required_cases_present,
        report.all_cases_pass,
        report.committed_artifact_review.all_committed_refs_exist,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.artifact_descriptor_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_artifact_descriptor_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarArticleTransformerArtifactValidationKind> {
    BTreeSet::from([
        TassadarArticleTransformerArtifactValidationKind::CanonicalDescriptorArtifactBacked,
        TassadarArticleTransformerArtifactValidationKind::TraceBoundDescriptorArtifactBacked,
        TassadarArticleTransformerArtifactValidationKind::ArtifactRoundtrip,
        TassadarArticleTransformerArtifactValidationKind::TensorInventoryMetadata,
        TassadarArticleTransformerArtifactValidationKind::RuntimeArtifactBinding,
    ])
}

fn case_rows() -> Vec<TassadarArticleTransformerArtifactCaseRow> {
    vec![
        canonical_descriptor_artifact_backed_case(),
        trace_bound_descriptor_artifact_backed_case(),
        artifact_roundtrip_case(),
        tensor_inventory_metadata_case(),
        runtime_artifact_binding_case(),
    ]
}

fn canonical_descriptor_artifact_backed_case() -> TassadarArticleTransformerArtifactCaseRow {
    match TassadarArticleTransformer::canonical_reference() {
        Ok(model)
            if model.descriptor().model.model_id == TassadarArticleTransformer::MODEL_ID
                && model.weight_metadata().format == WeightFormat::SafeTensors
                && model.weight_metadata().source == WeightSource::ExternalArtifact
                && !model.weight_metadata().artifacts.is_empty()
                && !model.weight_metadata().tensors.is_empty()
                && model.artifact_binding().weight_bundle_digest == model.weight_metadata().digest =>
        {
            case_row(
                "canonical_descriptor_artifact_backed",
                TassadarArticleTransformerArtifactValidationKind::CanonicalDescriptorArtifactBacked,
                true,
                format!(
                    "canonical descriptor now binds `{}` to safetensors-backed weight metadata with tensor_count={} and artifact_sha256={}",
                    model.descriptor().model.model_id,
                    model.weight_metadata().tensors.len(),
                    model.artifact_binding().primary_artifact_sha256
                ),
            )
        }
        Ok(model) => case_row(
            "canonical_descriptor_artifact_backed",
            TassadarArticleTransformerArtifactValidationKind::CanonicalDescriptorArtifactBacked,
            false,
            format!(
                "canonical descriptor used format={:?} source={:?} tensor_count={} artifact_count={} weight_bundle_match={}",
                model.weight_metadata().format,
                model.weight_metadata().source,
                model.weight_metadata().tensors.len(),
                model.weight_metadata().artifacts.len(),
                model.artifact_binding().weight_bundle_digest == model.weight_metadata().digest
            ),
        ),
        Err(error) => case_row(
            "canonical_descriptor_artifact_backed",
            TassadarArticleTransformerArtifactValidationKind::CanonicalDescriptorArtifactBacked,
            false,
            format!("failed to load canonical descriptor: {error}"),
        ),
    }
}

fn trace_bound_descriptor_artifact_backed_case() -> TassadarArticleTransformerArtifactCaseRow {
    match TassadarArticleTransformer::article_trace_domain_reference() {
        Ok(model)
            if model.descriptor().model.model_id == TassadarArticleTransformer::TRACE_BOUND_MODEL_ID
                && model.weight_metadata().format == WeightFormat::SafeTensors
                && model.weight_metadata().source == WeightSource::ExternalArtifact
                && !model.weight_metadata().artifacts.is_empty()
                && !model.weight_metadata().tensors.is_empty()
                && model.artifact_binding().weight_bundle_digest == model.weight_metadata().digest =>
        {
            case_row(
                "trace_bound_descriptor_artifact_backed",
                TassadarArticleTransformerArtifactValidationKind::TraceBoundDescriptorArtifactBacked,
                true,
                format!(
                    "trace-bound descriptor now binds tokenizer-sized config source_vocab={} target_vocab={} to one safetensors artifact digest {}",
                    model.descriptor().config.source_vocab_size,
                    model.descriptor().config.target_vocab_size,
                    model.weight_metadata().digest
                ),
            )
        }
        Ok(model) => case_row(
            "trace_bound_descriptor_artifact_backed",
            TassadarArticleTransformerArtifactValidationKind::TraceBoundDescriptorArtifactBacked,
            false,
            format!(
                "trace-bound descriptor used format={:?} source={:?} tensor_count={} artifact_count={} weight_bundle_match={}",
                model.weight_metadata().format,
                model.weight_metadata().source,
                model.weight_metadata().tensors.len(),
                model.weight_metadata().artifacts.len(),
                model.artifact_binding().weight_bundle_digest == model.weight_metadata().digest
            ),
        ),
        Err(error) => case_row(
            "trace_bound_descriptor_artifact_backed",
            TassadarArticleTransformerArtifactValidationKind::TraceBoundDescriptorArtifactBacked,
            false,
            format!("failed to load trace-bound descriptor: {error}"),
        ),
    }
}

fn artifact_roundtrip_case() -> TassadarArticleTransformerArtifactCaseRow {
    match (|| -> Result<(String, String), TassadarArticleTransformerError> {
        let model = TassadarArticleTransformer::canonical_reference()?;
        let roundtrip_root =
            std::env::temp_dir().join("psionic_tassadar_article_transformer_roundtrip");
        let descriptor_path = roundtrip_root.join("article_transformer_descriptor.json");
        let artifact_path = roundtrip_root.join("article_transformer_weights.safetensors");
        let written_descriptor = model.write_artifact_bundle(&descriptor_path, &artifact_path)?;
        let reloaded = TassadarArticleTransformer::load_from_descriptor_path(&descriptor_path)?;
        let _ = fs::remove_dir_all(&roundtrip_root);
        Ok((
            written_descriptor.stable_digest(),
            reloaded.descriptor().stable_digest(),
        ))
    })() {
        Ok((written_digest, reloaded_digest)) if written_digest == reloaded_digest => case_row(
            "artifact_roundtrip",
            TassadarArticleTransformerArtifactValidationKind::ArtifactRoundtrip,
            true,
            format!(
                "descriptor and safetensors roundtrip preserved stable digest `{written_digest}`"
            ),
        ),
        Ok((written_digest, reloaded_digest)) => case_row(
            "artifact_roundtrip",
            TassadarArticleTransformerArtifactValidationKind::ArtifactRoundtrip,
            false,
            format!(
                "roundtrip drifted descriptor digest from `{written_digest}` to `{reloaded_digest}`"
            ),
        ),
        Err(error) => case_row(
            "artifact_roundtrip",
            TassadarArticleTransformerArtifactValidationKind::ArtifactRoundtrip,
            false,
            format!("failed to roundtrip canonical descriptor and artifact: {error}"),
        ),
    }
}

fn tensor_inventory_metadata_case() -> TassadarArticleTransformerArtifactCaseRow {
    match (TassadarArticleTransformer::canonical_reference(), TassadarArticleTransformer::article_trace_domain_reference()) {
        (Ok(canonical), Ok(trace_bound))
            if tensor_inventory_is_well_formed(&canonical)
                && tensor_inventory_is_well_formed(&trace_bound) =>
        {
            case_row(
                "tensor_inventory_metadata",
                TassadarArticleTransformerArtifactValidationKind::TensorInventoryMetadata,
                true,
                format!(
                    "canonical tensor_count={} and trace-bound tensor_count={} both expose unique non-empty tensor metadata rows",
                    canonical.weight_metadata().tensors.len(),
                    trace_bound.weight_metadata().tensors.len(),
                ),
            )
        }
        (Ok(canonical), Ok(trace_bound)) => case_row(
            "tensor_inventory_metadata",
            TassadarArticleTransformerArtifactValidationKind::TensorInventoryMetadata,
            false,
            format!(
                "canonical tensor_count={} trace_bound tensor_count={} uniqueness_or_shape_check_failed",
                canonical.weight_metadata().tensors.len(),
                trace_bound.weight_metadata().tensors.len(),
            ),
        ),
        (Err(error), _) | (_, Err(error)) => case_row(
            "tensor_inventory_metadata",
            TassadarArticleTransformerArtifactValidationKind::TensorInventoryMetadata,
            false,
            format!("failed to load descriptor for tensor inventory review: {error}"),
        ),
    }
}

fn runtime_artifact_binding_case() -> TassadarArticleTransformerArtifactCaseRow {
    match TassadarArticleTransformer::canonical_reference() {
        Ok(model) => {
            let binding = model.model_artifact_binding();
            if binding.weight_bundle_digest == model.weight_metadata().digest
                && binding.artifact_id == model.artifact_binding().artifact_id
                && binding.primary_artifact_sha256
                    == model.artifact_binding().primary_artifact_sha256
            {
                case_row(
                    "runtime_artifact_binding",
                    TassadarArticleTransformerArtifactValidationKind::RuntimeArtifactBinding,
                    true,
                    format!(
                        "runtime binding now reuses weight_bundle_digest={} and artifact_id={}",
                        binding.weight_bundle_digest, binding.artifact_id
                    ),
                )
            } else {
                case_row(
                    "runtime_artifact_binding",
                    TassadarArticleTransformerArtifactValidationKind::RuntimeArtifactBinding,
                    false,
                    format!(
                        "runtime binding used artifact_id={} weight_bundle_digest={} primary_artifact_sha256={} but descriptor used artifact_id={} weight_bundle_digest={} primary_artifact_sha256={}",
                        binding.artifact_id,
                        binding.weight_bundle_digest,
                        binding.primary_artifact_sha256,
                        model.artifact_binding().artifact_id,
                        model.weight_metadata().digest,
                        model.artifact_binding().primary_artifact_sha256,
                    ),
                )
            }
        }
        Err(error) => case_row(
            "runtime_artifact_binding",
            TassadarArticleTransformerArtifactValidationKind::RuntimeArtifactBinding,
            false,
            format!("failed to load canonical descriptor for runtime binding review: {error}"),
        ),
    }
}

fn tensor_inventory_is_well_formed(model: &TassadarArticleTransformer) -> bool {
    let tensors = &model.weight_metadata().tensors;
    let unique_names = tensors
        .iter()
        .map(|tensor| tensor.name.as_str())
        .collect::<BTreeSet<_>>()
        .len()
        == tensors.len();
    unique_names
        && tensors.iter().all(|tensor| {
            !tensor.name.is_empty()
                && !tensor.shape.dims().is_empty()
                && tensor.shape.element_count() > 0
                && tensor.dtype == psionic_core::DType::F32
        })
        && model.artifact_binding().tensor_count == tensors.len()
}

fn committed_artifact_review() -> TassadarArticleTransformerCommittedArtifactReview {
    let canonical_descriptor_ref = TassadarArticleTransformer::CANONICAL_DESCRIPTOR_REF;
    let canonical_artifact_ref = TassadarArticleTransformer::CANONICAL_ARTIFACT_REF;
    let trace_bound_descriptor_ref = TassadarArticleTransformer::TRACE_BOUND_DESCRIPTOR_REF;
    let trace_bound_artifact_ref = TassadarArticleTransformer::TRACE_BOUND_ARTIFACT_REF;
    let review_refs = [
        canonical_descriptor_ref,
        canonical_artifact_ref,
        trace_bound_descriptor_ref,
        trace_bound_artifact_ref,
    ];
    let all_committed_refs_exist = review_refs
        .iter()
        .all(|relative| repo_root().join(relative).exists());
    let all_refs_under_fixtures_tassadar_models = review_refs
        .iter()
        .all(|relative| relative.starts_with("fixtures/tassadar/models/"));
    TassadarArticleTransformerCommittedArtifactReview {
        canonical_descriptor_ref: String::from(canonical_descriptor_ref),
        canonical_artifact_ref: String::from(canonical_artifact_ref),
        trace_bound_descriptor_ref: String::from(trace_bound_descriptor_ref),
        trace_bound_artifact_ref: String::from(trace_bound_artifact_ref),
        all_committed_refs_exist,
        all_refs_under_fixtures_tassadar_models,
        detail: format!(
            "committed article-transformer descriptor and safetensors refs now live under `fixtures/tassadar/models/` with canonical_exists={} trace_bound_exists={}",
            repo_root().join(canonical_descriptor_ref).exists()
                && repo_root().join(canonical_artifact_ref).exists(),
            repo_root().join(trace_bound_descriptor_ref).exists()
                && repo_root().join(trace_bound_artifact_ref).exists(),
        ),
    }
}

fn case_row(
    case_id: &str,
    validation_kind: TassadarArticleTransformerArtifactValidationKind,
    passed: bool,
    detail: impl Into<String>,
) -> TassadarArticleTransformerArtifactCaseRow {
    TassadarArticleTransformerArtifactCaseRow {
        case_id: String::from(case_id),
        validation_kind,
        passed,
        detail: detail.into(),
    }
}

pub fn tassadar_article_transformer_artifact_descriptor_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_ARTIFACT_DESCRIPTOR_REPORT_REF)
}

pub fn write_tassadar_article_transformer_artifact_descriptor_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerArtifactDescriptorReport,
    TassadarArticleTransformerArtifactDescriptorReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerArtifactDescriptorReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_artifact_descriptor_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerArtifactDescriptorReportError::Write {
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
) -> Result<T, TassadarArticleTransformerArtifactDescriptorReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleTransformerArtifactDescriptorReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerArtifactDescriptorReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_report_from_inputs, build_tassadar_article_transformer_artifact_descriptor_report,
        committed_artifact_review, read_json,
        tassadar_article_transformer_artifact_descriptor_report_path,
        write_tassadar_article_transformer_artifact_descriptor_report,
        TassadarArticleTransformerArtifactCaseRow,
        TassadarArticleTransformerArtifactDescriptorReport,
        TassadarArticleTransformerArtifactValidationKind,
        TASSADAR_ARTICLE_TRANSFORMER_ARTIFACT_DESCRIPTOR_REPORT_REF,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        TassadarArticleEquivalenceAcceptanceStatus,
    };

    #[test]
    fn article_transformer_artifact_descriptor_tracks_green_requirement_without_final_green() {
        let report =
            build_tassadar_article_transformer_artifact_descriptor_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.acceptance_gate_tie.acceptance_status,
            TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert!(report.committed_artifact_review.all_committed_refs_exist);
        assert!(report.all_required_cases_present);
        assert!(report.all_cases_pass);
        assert!(report.artifact_descriptor_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_rows.len(), 5);
    }

    #[test]
    fn failed_case_keeps_artifact_descriptor_contract_red() {
        let acceptance_gate =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("acceptance gate");
        let mut case_rows = build_tassadar_article_transformer_artifact_descriptor_report()
            .expect("report")
            .case_rows;
        case_rows[0] = TassadarArticleTransformerArtifactCaseRow {
            case_id: String::from("canonical_descriptor_artifact_backed"),
            validation_kind:
                TassadarArticleTransformerArtifactValidationKind::CanonicalDescriptorArtifactBacked,
            passed: false,
            detail: String::from("mutated red path"),
        };

        let report =
            build_report_from_inputs(acceptance_gate, committed_artifact_review(), case_rows);
        assert!(!report.all_cases_pass);
        assert!(!report.artifact_descriptor_contract_green);
        assert!(!report.article_equivalence_green);
    }

    #[test]
    fn article_transformer_artifact_descriptor_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_artifact_descriptor_report().expect("report");
        let committed: TassadarArticleTransformerArtifactDescriptorReport =
            read_json(tassadar_article_transformer_artifact_descriptor_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_transformer_artifact_descriptor_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_artifact_descriptor_report.json");
        let written = write_tassadar_article_transformer_artifact_descriptor_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleTransformerArtifactDescriptorReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_transformer_artifact_descriptor_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_transformer_artifact_descriptor_report.json")
        );
        assert_eq!(
            TASSADAR_ARTICLE_TRANSFORMER_ARTIFACT_DESCRIPTOR_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_article_transformer_artifact_descriptor_report.json"
        );
    }
}
