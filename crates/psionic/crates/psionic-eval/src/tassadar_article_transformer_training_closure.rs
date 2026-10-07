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

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    read_tassadar_article_transformer_training_evidence_bundle,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleTransformerTrainingEvidenceBundle,
    TassadarArticleTransformerTrainingEvidenceError,
    TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRANSFORMER_TRAINING_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_transformer_training_closure_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-164";
const EXPECTED_MODEL_MODULE_REF: &str = "crates/psionic-models/src/tassadar_article_transformer.rs";
const EXPECTED_TRANSFORMER_MODULE_REF: &str =
    "crates/psionic-transformer/src/encoder_decoder.rs";
const EXPECTED_TRAIN_MODULE_REF: &str =
    "crates/psionic-train/src/tassadar_article_transformer_training.rs";
const MODELS_CARGO_TOML_REF: &str = "crates/psionic-models/Cargo.toml";
const TRAIN_CARGO_TOML_REF: &str = "crates/psionic-train/Cargo.toml";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTransformerTrainingValidationKind {
    RecipeSurface,
    TinyBatchOverfit,
    DeterministicCheckpointRestore,
    FiniteGradientChecks,
    BoundaryRootedInTransformer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTrainingCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarArticleTransformerTrainingValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTrainingBoundaryReview {
    pub model_module_ref: String,
    pub transformer_module_ref: String,
    pub train_module_ref: String,
    pub models_cargo_toml_ref: String,
    pub train_cargo_toml_ref: String,
    pub evidence_roots_in_expected_modules: bool,
    pub models_depend_on_psionic_transformer: bool,
    pub train_depends_on_psionic_transformer: bool,
    pub canonical_boundary_green: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTrainingAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub evidence_bundle_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TassadarArticleTransformerTrainingClosureReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleTransformerTrainingAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub evidence_bundle_ref: String,
    pub evidence_bundle: TassadarArticleTransformerTrainingEvidenceBundle,
    pub case_rows: Vec<TassadarArticleTransformerTrainingCaseRow>,
    pub boundary_review: TassadarArticleTransformerTrainingBoundaryReview,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub article_transformer_training_contract_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTransformerTrainingClosureReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    CanonicalBoundary(#[from] TassadarCanonicalTransformerStackBoundaryReportError),
    #[error(transparent)]
    Evidence(#[from] TassadarArticleTransformerTrainingEvidenceError),
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

pub fn build_tassadar_article_transformer_training_closure_report() -> Result<
    TassadarArticleTransformerTrainingClosureReport,
    TassadarArticleTransformerTrainingClosureReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let canonical_boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let evidence_bundle = read_tassadar_article_transformer_training_evidence_bundle(
        TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF,
    )?;
    let boundary_review = boundary_review(&evidence_bundle, &canonical_boundary_report)?;
    let case_rows = case_rows(&evidence_bundle, &boundary_review);
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
    evidence_bundle: TassadarArticleTransformerTrainingEvidenceBundle,
    case_rows: Vec<TassadarArticleTransformerTrainingCaseRow>,
    boundary_review: TassadarArticleTransformerTrainingBoundaryReview,
) -> TassadarArticleTransformerTrainingClosureReport {
    let acceptance_gate_tie = TassadarArticleTransformerTrainingAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        evidence_bundle_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF),
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
    let article_transformer_training_contract_green = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && all_required_cases_present
        && all_cases_pass
        && boundary_review.passed;
    let article_equivalence_green = article_transformer_training_contract_green
        && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTransformerTrainingClosureReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_transformer_training_closure.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        evidence_bundle_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_TRAINING_EVIDENCE_BUNDLE_REF),
        evidence_bundle,
        case_rows,
        boundary_review,
        all_required_cases_present,
        all_cases_pass,
        article_transformer_training_contract_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report closes only the bounded article-Transformer training recipe on toy sequence tasks. It proves that the canonical owned stack can train, emit finite gradients, overfit a tiny batch, and restore deterministically while still routing through `psionic-transformer`. It does not claim full article-model training, benchmark parity, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article Transformer training closure now records case_rows={}, all_required_cases_present={}, all_cases_pass={}, tied_requirement_satisfied={}, training_contract_green={}, and article_equivalence_green={}.",
        report.case_rows.len(),
        report.all_required_cases_present,
        report.all_cases_pass,
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.article_transformer_training_contract_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_transformer_training_closure_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarArticleTransformerTrainingValidationKind> {
    BTreeSet::from([
        TassadarArticleTransformerTrainingValidationKind::RecipeSurface,
        TassadarArticleTransformerTrainingValidationKind::TinyBatchOverfit,
        TassadarArticleTransformerTrainingValidationKind::DeterministicCheckpointRestore,
        TassadarArticleTransformerTrainingValidationKind::FiniteGradientChecks,
        TassadarArticleTransformerTrainingValidationKind::BoundaryRootedInTransformer,
    ])
}

fn case_rows(
    evidence_bundle: &TassadarArticleTransformerTrainingEvidenceBundle,
    boundary_review: &TassadarArticleTransformerTrainingBoundaryReview,
) -> Vec<TassadarArticleTransformerTrainingCaseRow> {
    vec![
        recipe_surface_case(evidence_bundle),
        tiny_batch_overfit_case(evidence_bundle),
        deterministic_checkpoint_restore_case(evidence_bundle),
        finite_gradient_checks_case(evidence_bundle),
        boundary_rooted_in_transformer_case(boundary_review),
    ]
}

fn recipe_surface_case(
    evidence_bundle: &TassadarArticleTransformerTrainingEvidenceBundle,
) -> TassadarArticleTransformerTrainingCaseRow {
    let passed = evidence_bundle.loss_kind == "label_smoothed_cross_entropy"
        && evidence_bundle.optimizer_kind == "adam"
        && evidence_bundle.scheduler_kind == "inverse_square_root_warmup"
        && evidence_bundle.warmup_steps > 0
        && evidence_bundle.label_smoothing > 0.0
        && evidence_bundle.label_smoothing < 1.0
        && !evidence_bundle.training_examples.is_empty()
        && !evidence_bundle.held_out_examples.is_empty()
        && !evidence_bundle.step_evidence.is_empty()
        && !evidence_bundle.trainable_parameter_ids.is_empty();
    case_row(
        "recipe_surface",
        TassadarArticleTransformerTrainingValidationKind::RecipeSurface,
        passed,
        format!(
            "loss_kind={}, optimizer_kind={}, scheduler_kind={}, warmup_steps={}, label_smoothing={}, training_examples={}, held_out_examples={}, step_evidence_rows={}",
            evidence_bundle.loss_kind,
            evidence_bundle.optimizer_kind,
            evidence_bundle.scheduler_kind,
            evidence_bundle.warmup_steps,
            evidence_bundle.label_smoothing,
            evidence_bundle.training_examples.len(),
            evidence_bundle.held_out_examples.len(),
            evidence_bundle.step_evidence.len(),
        ),
    )
}

fn tiny_batch_overfit_case(
    evidence_bundle: &TassadarArticleTransformerTrainingEvidenceBundle,
) -> TassadarArticleTransformerTrainingCaseRow {
    let passed = evidence_bundle.overfit_training_exact_match
        && evidence_bundle.final_training_exact_match_count == evidence_bundle.training_examples.len()
        && evidence_bundle.final_training_mean_loss < evidence_bundle.initial_training_mean_loss;
    case_row(
        "tiny_batch_overfit",
        TassadarArticleTransformerTrainingValidationKind::TinyBatchOverfit,
        passed,
        format!(
            "training_exact_match={}/{} initial_mean_loss={} final_mean_loss={}",
            evidence_bundle.final_training_exact_match_count,
            evidence_bundle.training_examples.len(),
            evidence_bundle.initial_training_mean_loss,
            evidence_bundle.final_training_mean_loss,
        ),
    )
}

fn deterministic_checkpoint_restore_case(
    evidence_bundle: &TassadarArticleTransformerTrainingEvidenceBundle,
) -> TassadarArticleTransformerTrainingCaseRow {
    let checkpoint = &evidence_bundle.checkpoint;
    let passed = checkpoint.restore_matches_trained_state
        && !checkpoint.manifest_digest.trim().is_empty()
        && checkpoint.trained_trainable_parameter_digest
            == checkpoint.restored_trainable_parameter_digest;
    case_row(
        "deterministic_checkpoint_restore",
        TassadarArticleTransformerTrainingValidationKind::DeterministicCheckpointRestore,
        passed,
        format!(
            "checkpoint_ref={} manifest_digest={} restore_matches_trained_state={}",
            checkpoint.checkpoint_ref,
            checkpoint.manifest_digest,
            checkpoint.restore_matches_trained_state,
        ),
    )
}

fn finite_gradient_checks_case(
    evidence_bundle: &TassadarArticleTransformerTrainingEvidenceBundle,
) -> TassadarArticleTransformerTrainingCaseRow {
    let all_finite = !evidence_bundle.gradient_checks.is_empty()
        && evidence_bundle.gradient_checks.iter().all(|row| row.all_finite);
    let any_non_zero = evidence_bundle
        .gradient_checks
        .iter()
        .any(|row| row.max_abs_gradient.is_finite() && row.max_abs_gradient > 0.0);
    case_row(
        "finite_gradient_checks",
        TassadarArticleTransformerTrainingValidationKind::FiniteGradientChecks,
        all_finite && any_non_zero,
        format!(
            "gradient_checks={} all_finite={} any_non_zero={}",
            evidence_bundle.gradient_checks.len(),
            all_finite,
            any_non_zero,
        ),
    )
}

fn boundary_rooted_in_transformer_case(
    boundary_review: &TassadarArticleTransformerTrainingBoundaryReview,
) -> TassadarArticleTransformerTrainingCaseRow {
    case_row(
        "boundary_rooted_in_transformer",
        TassadarArticleTransformerTrainingValidationKind::BoundaryRootedInTransformer,
        boundary_review.passed,
        boundary_review.detail.clone(),
    )
}

fn boundary_review(
    evidence_bundle: &TassadarArticleTransformerTrainingEvidenceBundle,
    canonical_boundary_report: &TassadarCanonicalTransformerStackBoundaryReport,
) -> Result<
    TassadarArticleTransformerTrainingBoundaryReview,
    TassadarArticleTransformerTrainingClosureReportError,
> {
    let evidence_roots_in_expected_modules = evidence_bundle.model_module_ref
        == EXPECTED_MODEL_MODULE_REF
        && evidence_bundle.transformer_module_ref == EXPECTED_TRANSFORMER_MODULE_REF
        && evidence_bundle.train_module_ref == EXPECTED_TRAIN_MODULE_REF;
    let models_depend_on_psionic_transformer =
        cargo_toml_contains_dependency(MODELS_CARGO_TOML_REF, "psionic-transformer")?;
    let train_depends_on_psionic_transformer =
        cargo_toml_contains_dependency(TRAIN_CARGO_TOML_REF, "psionic-transformer")?;
    let canonical_boundary_green = canonical_boundary_report.boundary_contract_green;
    let passed = evidence_roots_in_expected_modules
        && models_depend_on_psionic_transformer
        && train_depends_on_psionic_transformer
        && canonical_boundary_green;
    Ok(TassadarArticleTransformerTrainingBoundaryReview {
        model_module_ref: evidence_bundle.model_module_ref.clone(),
        transformer_module_ref: evidence_bundle.transformer_module_ref.clone(),
        train_module_ref: evidence_bundle.train_module_ref.clone(),
        models_cargo_toml_ref: String::from(MODELS_CARGO_TOML_REF),
        train_cargo_toml_ref: String::from(TRAIN_CARGO_TOML_REF),
        evidence_roots_in_expected_modules,
        models_depend_on_psionic_transformer,
        train_depends_on_psionic_transformer,
        canonical_boundary_green,
        passed,
        detail: format!(
            "evidence roots model={} transformer={} train={}, cargo deps model->transformer={} train->transformer={}, canonical_boundary_green={}",
            evidence_roots_in_expected_modules,
            evidence_bundle.transformer_module_ref,
            evidence_bundle.train_module_ref,
            models_depend_on_psionic_transformer,
            train_depends_on_psionic_transformer,
            canonical_boundary_green,
        ),
    })
}

fn cargo_toml_contains_dependency(
    relative_path: &str,
    dependency_name: &str,
) -> Result<bool, TassadarArticleTransformerTrainingClosureReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleTransformerTrainingClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.contains(dependency_name))
}

fn case_row(
    case_id: &str,
    validation_kind: TassadarArticleTransformerTrainingValidationKind,
    passed: bool,
    detail: impl Into<String>,
) -> TassadarArticleTransformerTrainingCaseRow {
    TassadarArticleTransformerTrainingCaseRow {
        case_id: String::from(case_id),
        validation_kind,
        passed,
        detail: detail.into(),
    }
}

pub fn tassadar_article_transformer_training_closure_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRANSFORMER_TRAINING_CLOSURE_REPORT_REF)
}

pub fn write_tassadar_article_transformer_training_closure_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTransformerTrainingClosureReport,
    TassadarArticleTransformerTrainingClosureReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTransformerTrainingClosureReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_transformer_training_closure_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTransformerTrainingClosureReportError::Write {
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
) -> Result<T, TassadarArticleTransformerTrainingClosureReportError> {
    let path = path.as_ref();
    let bytes = fs::read(path).map_err(|error| {
        TassadarArticleTransformerTrainingClosureReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTransformerTrainingClosureReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_transformer_training_closure_report, read_json,
        tassadar_article_transformer_training_closure_report_path,
        write_tassadar_article_transformer_training_closure_report,
        TassadarArticleTransformerTrainingClosureReport,
        TASSADAR_ARTICLE_TRANSFORMER_TRAINING_CLOSURE_REPORT_REF,
    };

    #[test]
    fn article_transformer_training_closure_tracks_green_training_lane_without_final_green() {
        let report =
            build_tassadar_article_transformer_training_closure_report().expect("closure report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.acceptance_gate_tie.acceptance_status,
            crate::TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert!(report.all_required_cases_present);
        assert!(report.all_cases_pass);
        assert!(report.boundary_review.passed);
        assert!(report.article_transformer_training_contract_green);
        assert!(report.article_equivalence_green);
        assert_eq!(report.case_rows.len(), 5);
    }

    #[test]
    fn article_transformer_training_closure_matches_committed_truth() {
        let generated =
            build_tassadar_article_transformer_training_closure_report().expect("closure report");
        let committed: TassadarArticleTransformerTrainingClosureReport = read_json(
            tassadar_article_transformer_training_closure_report_path(),
        )
        .expect("committed closure report");
        assert_eq!(generated, committed);
        assert_eq!(
            TASSADAR_ARTICLE_TRANSFORMER_TRAINING_CLOSURE_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_article_transformer_training_closure_report.json"
        );
    }

    #[test]
    fn write_article_transformer_training_closure_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_transformer_training_closure_report.json");
        let written = write_tassadar_article_transformer_training_closure_report(&output_path)
            .expect("write closure report");
        let persisted: TassadarArticleTransformerTrainingClosureReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
    }
}
