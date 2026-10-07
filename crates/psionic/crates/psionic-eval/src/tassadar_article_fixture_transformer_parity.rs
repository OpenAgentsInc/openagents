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

use psionic_core::Shape;
use psionic_models::{
    TassadarArticleTransformer, TassadarArticleTransformerError,
    TassadarArticleTransformerTraceDomainBatch, TassadarArticleTransformerTraceDomainRoundtrip,
    TassadarExecutorFixture,
};
use psionic_runtime::{
    tassadar_article_class_corpus, TassadarArticleTransformerForwardPassEvidenceBundle,
    TassadarArticleTransformerModelArtifactBinding, TassadarExecution, TassadarExecutorDecodeMode,
    TassadarFixtureRunner, TassadarProgramArtifact, TassadarTraceDiffReport,
    TassadarValidationCase, TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF,
    TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF,
};
use psionic_transformer::TransformerExecutionMode;

use crate::{
    build_tassadar_article_class_suite, build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    read_tassadar_article_transformer_weight_lineage_contract,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleTransformerWeightLineageContract,
    TassadarArticleTransformerWeightLineageError, TassadarBenchmarkError,
    TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_fixture_transformer_parity_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-170";
const ARTICLE_SUITE_VERSION: &str = "2026.03.17";
const FORWARD_PRODUCT_ID: &str = "psionic.article_transformer.fixture_parity";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFixtureTransformerParityAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub benchmark_report_ref: String,
    pub lineage_contract_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFixtureTransformerParityCaseRow {
    pub case_id: String,
    pub program_id: String,
    pub artifact_id: String,
    pub requested_decode_mode: TassadarExecutorDecodeMode,
    pub fixture_selection_state: String,
    pub fixture_effective_decode_mode: Option<TassadarExecutorDecodeMode>,
    pub fixture_selection_detail: String,
    pub fixture_trace_step_count: usize,
    pub prompt_token_count: usize,
    pub target_token_count: usize,
    pub output_count: usize,
    pub fixture_trace_digest: String,
    pub roundtrip_trace_digest: String,
    pub fixture_behavior_digest: String,
    pub roundtrip_behavior_digest: String,
    pub trace_diff_report: TassadarTraceDiffReport,
    pub forward_bundle_digest: String,
    pub forward_trace_artifact_digest: String,
    pub fixture_routeable: bool,
    pub transformer_routeable: bool,
    pub within_transformer_context_window: bool,
    pub prompt_boundary_preserved: bool,
    pub halt_marker_preserved: bool,
    pub roundtrip_exact: bool,
    pub trace_shape_parity: bool,
    pub output_parity: bool,
    pub behavior_parity: bool,
    pub forward_binding_parity: bool,
    pub forward_replay_stable: bool,
    pub case_passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFixtureTransformerParityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleFixtureTransformerParityAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub benchmark_report_ref: String,
    pub article_suite_version: String,
    pub declared_case_ids: Vec<String>,
    pub article_suite_corpus_digest: String,
    pub lineage_contract_ref: String,
    pub lineage_contract_digest: String,
    pub fixture_model_id: String,
    pub fixture_model_descriptor_digest: String,
    pub transformer_model_artifact: TassadarArticleTransformerModelArtifactBinding,
    pub transformer_model_matches_lineage_contract: bool,
    pub case_rows: Vec<TassadarArticleFixtureTransformerParityCaseRow>,
    pub supported_case_count: usize,
    pub routeable_case_count: usize,
    pub exact_trace_case_count: usize,
    pub exact_output_case_count: usize,
    pub context_window_fit_case_count: usize,
    pub forward_binding_case_count: usize,
    pub mismatch_case_ids: Vec<String>,
    pub all_declared_cases_present: bool,
    pub all_cases_pass: bool,
    pub replacement_certified: bool,
    pub replacement_publication_allowed: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleFixtureTransformerParityError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Benchmark(#[from] TassadarBenchmarkError),
    #[error(transparent)]
    CanonicalBoundary(#[from] TassadarCanonicalTransformerStackBoundaryReportError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error(transparent)]
    WeightLineage(#[from] TassadarArticleTransformerWeightLineageError),
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

pub fn build_tassadar_article_fixture_transformer_parity_report() -> Result<
    TassadarArticleFixtureTransformerParityReport,
    TassadarArticleFixtureTransformerParityError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let canonical_boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let suite = build_tassadar_article_class_suite(ARTICLE_SUITE_VERSION)?;
    let declared_cases = tassadar_article_class_corpus();
    let declared_case_ids = declared_cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let lineage_contract = read_tassadar_article_transformer_weight_lineage_contract(
        TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
    )?;
    let fixture = TassadarExecutorFixture::article_i32_compute_v1();
    let transformer_model = TassadarArticleTransformer::trained_trace_domain_reference()?;
    let expected_environment_refs = expected_forward_environment_refs();
    let transformer_model_artifact = transformer_model.model_artifact_binding();
    let transformer_model_matches_lineage_contract =
        transformer_model_artifact == lineage_contract.produced_model_artifact_binding;

    let case_rows = declared_cases
        .iter()
        .zip(suite.artifacts.iter())
        .map(|(case, artifact)| {
            build_case_row(
                case,
                artifact,
                &fixture,
                &transformer_model,
                &transformer_model_artifact,
                expected_environment_refs.as_slice(),
                true,
            )
        })
        .collect::<Vec<_>>();

    Ok(build_report_from_inputs(
        acceptance_gate_report,
        canonical_boundary_report,
        suite.corpus_digest,
        declared_case_ids,
        lineage_contract,
        &fixture,
        transformer_model_artifact,
        transformer_model_matches_lineage_contract,
        case_rows,
    ))
}

pub fn build_tassadar_article_fixture_transformer_parity_case_rows_for_model(
    transformer_model: &TassadarArticleTransformer,
    expected_environment_refs: &[String],
) -> Result<
    (
        Vec<TassadarArticleFixtureTransformerParityCaseRow>,
        Vec<String>,
        String,
        TassadarExecutorFixture,
        TassadarArticleTransformerModelArtifactBinding,
    ),
    TassadarArticleFixtureTransformerParityError,
> {
    build_case_rows_for_model(transformer_model, expected_environment_refs, true)
}

pub fn build_tassadar_article_fixture_transformer_behavior_case_rows_for_model(
    transformer_model: &TassadarArticleTransformer,
) -> Result<
    (
        Vec<TassadarArticleFixtureTransformerParityCaseRow>,
        Vec<String>,
        String,
        TassadarExecutorFixture,
        TassadarArticleTransformerModelArtifactBinding,
    ),
    TassadarArticleFixtureTransformerParityError,
> {
    build_case_rows_for_model(transformer_model, &[], false)
}

fn build_case_rows_for_model(
    transformer_model: &TassadarArticleTransformer,
    expected_environment_refs: &[String],
    include_forward_evidence: bool,
) -> Result<
    (
        Vec<TassadarArticleFixtureTransformerParityCaseRow>,
        Vec<String>,
        String,
        TassadarExecutorFixture,
        TassadarArticleTransformerModelArtifactBinding,
    ),
    TassadarArticleFixtureTransformerParityError,
> {
    let suite = build_tassadar_article_class_suite(ARTICLE_SUITE_VERSION)?;
    let declared_cases = tassadar_article_class_corpus();
    let declared_case_ids = declared_cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let fixture = TassadarExecutorFixture::article_i32_compute_v1();
    let transformer_model_artifact = transformer_model.model_artifact_binding();
    let case_rows = declared_cases
        .iter()
        .zip(suite.artifacts.iter())
        .map(|(case, artifact)| {
            build_case_row(
                case,
                artifact,
                &fixture,
                transformer_model,
                &transformer_model_artifact,
                expected_environment_refs,
                include_forward_evidence,
            )
        })
        .collect::<Vec<_>>();
    Ok((
        case_rows,
        declared_case_ids,
        suite.corpus_digest,
        fixture,
        transformer_model_artifact,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    article_suite_corpus_digest: String,
    declared_case_ids: Vec<String>,
    lineage_contract: TassadarArticleTransformerWeightLineageContract,
    fixture: &TassadarExecutorFixture,
    transformer_model_artifact: TassadarArticleTransformerModelArtifactBinding,
    transformer_model_matches_lineage_contract: bool,
    case_rows: Vec<TassadarArticleFixtureTransformerParityCaseRow>,
) -> TassadarArticleFixtureTransformerParityReport {
    let acceptance_gate_tie = TassadarArticleFixtureTransformerParityAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        lineage_contract_ref: String::from(
            TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate_report
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate_report.acceptance_status,
        blocked_issue_ids: acceptance_gate_report.blocked_issue_ids.clone(),
    };
    let supported_case_count = declared_case_ids.len();
    let routeable_case_count = case_rows
        .iter()
        .filter(|row| row.fixture_routeable && row.transformer_routeable)
        .count();
    let exact_trace_case_count = case_rows
        .iter()
        .filter(|row| row.trace_shape_parity)
        .count();
    let exact_output_case_count = case_rows
        .iter()
        .filter(|row| row.output_parity && row.behavior_parity)
        .count();
    let context_window_fit_case_count = case_rows
        .iter()
        .filter(|row| row.within_transformer_context_window)
        .count();
    let forward_binding_case_count = case_rows
        .iter()
        .filter(|row| {
            row.within_transformer_context_window
                && row.forward_binding_parity
                && row.forward_replay_stable
        })
        .count();
    let mismatch_case_ids = case_rows
        .iter()
        .filter(|row| !row.case_passed)
        .map(|row| row.case_id.clone())
        .collect::<Vec<_>>();
    let all_declared_cases_present = case_rows.len() == supported_case_count
        && case_rows.len() > 0
        && case_rows
            .iter()
            .map(|row| row.case_id.clone())
            .collect::<BTreeSet<_>>()
            == declared_case_ids.iter().cloned().collect::<BTreeSet<_>>();
    let all_cases_pass = mismatch_case_ids.is_empty()
        && supported_case_count == routeable_case_count
        && supported_case_count == exact_trace_case_count
        && supported_case_count == exact_output_case_count;
    let replacement_certified = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && transformer_model_matches_lineage_contract
        && all_declared_cases_present
        && all_cases_pass;
    let article_equivalence_green =
        replacement_certified && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleFixtureTransformerParityReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_fixture_transformer_parity.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        benchmark_report_ref: String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        article_suite_version: String::from(ARTICLE_SUITE_VERSION),
        declared_case_ids,
        article_suite_corpus_digest,
        lineage_contract_ref: String::from(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF),
        lineage_contract_digest: lineage_contract.contract_digest,
        fixture_model_id: fixture.descriptor().model.model_id.clone(),
        fixture_model_descriptor_digest: fixture.descriptor().stable_digest(),
        transformer_model_artifact,
        transformer_model_matches_lineage_contract,
        case_rows,
        supported_case_count,
        routeable_case_count,
        exact_trace_case_count,
        exact_output_case_count,
        context_window_fit_case_count,
        forward_binding_case_count,
        mismatch_case_ids,
        all_declared_cases_present,
        all_cases_pass,
        replacement_certified,
        replacement_publication_allowed: replacement_certified,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report certifies only that the committed trained trace-bound Transformer-backed article wrapper can inherit the old fixture lane's declared bounded article workload set without trace or terminal-state drift when the canonical fixture execution is threaded through the shared Transformer trace-domain wrapper over the full article corpus. It keeps forward-pass evidence checks explicit only on the cases that fit the current model window, and it does not yet claim zero-tool direct-proof ownership, reference-linear exactness from model weights alone, fast-route promotion, benchmark parity, single-run closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article fixture-to-Transformer parity now records supported_case_count={}, routeable_case_count={}, exact_trace_case_count={}, exact_output_case_count={}, context_window_fit_case_count={}, forward_binding_case_count={}, replacement_certified={}, and article_equivalence_green={}.",
        report.supported_case_count,
        report.routeable_case_count,
        report.exact_trace_case_count,
        report.exact_output_case_count,
        report.context_window_fit_case_count,
        report.forward_binding_case_count,
        report.replacement_certified,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_fixture_transformer_parity_report|",
        &report,
    );
    report
}

fn build_case_row(
    case: &TassadarValidationCase,
    artifact: &TassadarProgramArtifact,
    fixture: &TassadarExecutorFixture,
    transformer_model: &TassadarArticleTransformer,
    expected_model_artifact_binding: &TassadarArticleTransformerModelArtifactBinding,
    expected_environment_refs: &[String],
    include_forward_evidence: bool,
) -> TassadarArticleFixtureTransformerParityCaseRow {
    let requested_decode_mode = TassadarExecutorDecodeMode::ReferenceLinear;
    let selection = fixture.runtime_selection_diagnostic(&case.program, requested_decode_mode);
    let mut row = TassadarArticleFixtureTransformerParityCaseRow {
        case_id: case.case_id.clone(),
        program_id: case.program.program_id.clone(),
        artifact_id: artifact.artifact_id.clone(),
        requested_decode_mode,
        fixture_selection_state: format!("{:?}", selection.selection_state).to_lowercase(),
        fixture_effective_decode_mode: selection.effective_decode_mode,
        fixture_selection_detail: selection.detail.clone(),
        fixture_trace_step_count: 0,
        prompt_token_count: 0,
        target_token_count: 0,
        output_count: 0,
        fixture_trace_digest: String::new(),
        roundtrip_trace_digest: String::new(),
        fixture_behavior_digest: String::new(),
        roundtrip_behavior_digest: String::new(),
        trace_diff_report: empty_trace_diff_report(),
        forward_bundle_digest: String::new(),
        forward_trace_artifact_digest: String::new(),
        fixture_routeable: selection.effective_decode_mode == Some(requested_decode_mode),
        transformer_routeable: false,
        within_transformer_context_window: false,
        prompt_boundary_preserved: false,
        halt_marker_preserved: false,
        roundtrip_exact: false,
        trace_shape_parity: false,
        output_parity: false,
        behavior_parity: false,
        forward_binding_parity: false,
        forward_replay_stable: false,
        case_passed: false,
        detail: String::new(),
    };

    if artifact.validated_program.program_id != case.program.program_id
        || artifact.validated_program_digest != case.program.program_digest()
        || artifact.wasm_profile_id != case.program.profile_id
    {
        row.detail = format!(
            "canonical artifact `{}` drifted from article case `{}`",
            artifact.artifact_id, case.case_id
        );
        return row;
    }

    if !row.fixture_routeable {
        row.detail = format!(
            "fixture reference-linear route did not stay routeable on canonical article case `{}`: {}",
            case.case_id, row.fixture_selection_detail
        );
        return row;
    }

    let fixture_execution = match TassadarFixtureRunner::for_program(&case.program)
        .and_then(|runner| runner.execute(&case.program))
    {
        Ok(execution) => execution,
        Err(error) => {
            row.detail = format!(
                "fixture reference-linear execution failed on canonical article case `{}`: {}",
                case.case_id, error
            );
            return row;
        }
    };
    row.fixture_trace_step_count = fixture_execution.steps.len();
    row.output_count = fixture_execution.outputs.len();
    row.fixture_trace_digest = fixture_execution.trace_digest();
    row.fixture_behavior_digest = fixture_execution.behavior_digest();

    let roundtrip = match transformer_model
        .roundtrip_article_trace_domain_unbounded(&case.program, &fixture_execution)
    {
        Ok(roundtrip) => roundtrip,
        Err(error) => {
            row.detail = format!(
                "Transformer trace-domain wrapper failed to encode canonical article case `{}`: {}",
                case.case_id, error
            );
            return row;
        }
    };
    row.transformer_routeable = true;
    row.prompt_token_count = roundtrip.batch.prompt_token_count;
    row.target_token_count = roundtrip.batch.target_token_count;
    row.prompt_boundary_preserved = roundtrip.prompt_boundary_preserved;
    row.halt_marker_preserved = roundtrip.halt_marker_preserved;
    row.roundtrip_exact = roundtrip.roundtrip_exact;

    let roundtrip_execution = roundtrip.decoded_trace.materialize_execution(
        fixture_execution.program_id.clone(),
        fixture_execution.profile_id.clone(),
        fixture_execution.runner_id.clone(),
        fixture_execution.trace_abi.clone(),
    );
    row.roundtrip_trace_digest = roundtrip_execution.trace_digest();
    row.roundtrip_behavior_digest = roundtrip_execution.behavior_digest();
    row.trace_diff_report =
        TassadarTraceDiffReport::from_executions(&fixture_execution, &roundtrip_execution);
    row.trace_shape_parity = trace_shape_parity(&roundtrip, &row.trace_diff_report);
    row.output_parity = execution_output_parity(&fixture_execution, &roundtrip_execution);
    row.behavior_parity = row.fixture_behavior_digest == row.roundtrip_behavior_digest;
    row.within_transformer_context_window =
        within_transformer_context_window(transformer_model, &roundtrip.batch);

    if include_forward_evidence && row.within_transformer_context_window {
        let forward_evidence = match transformer_model.forward_with_runtime_evidence(
            format!(
                "tassadar.article_transformer.fixture_parity.run.{}",
                case.case_id
            ),
            format!(
                "tassadar.article_transformer.fixture_parity.request.{}",
                case.case_id
            ),
            FORWARD_PRODUCT_ID,
            expected_environment_refs.to_vec(),
            Shape::new(roundtrip.batch.source_shape.clone()),
            &roundtrip.batch.source_token_ids,
            Shape::new(roundtrip.batch.target_shape.clone()),
            &roundtrip.batch.target_token_ids,
            TransformerExecutionMode::Eval,
            None,
        ) {
            Ok(evidence) => evidence,
            Err(error) => {
                row.detail = format!(
                    "Transformer forward evidence failed on canonical article case `{}`: {}",
                    case.case_id, error
                );
                return row;
            }
        };
        row.forward_bundle_digest = forward_evidence.bundle_digest.clone();
        row.forward_trace_artifact_digest = forward_evidence.trace_artifact.artifact_digest.clone();
        row.forward_binding_parity = forward_binding_parity(
            &roundtrip.batch,
            &forward_evidence,
            expected_model_artifact_binding,
            expected_environment_refs,
        );
        row.forward_replay_stable = forward_evidence.replay_receipt.deterministic_match;
    }
    let behavior_parity_passed = row.fixture_routeable
        && row.transformer_routeable
        && row.trace_shape_parity
        && row.output_parity
        && row.behavior_parity;
    row.case_passed = behavior_parity_passed
        && (!include_forward_evidence
            || !row.within_transformer_context_window
            || (row.forward_binding_parity && row.forward_replay_stable));
    row.detail = if row.case_passed {
        if include_forward_evidence && row.within_transformer_context_window {
            format!(
                "fixture reference-linear execution and the trained trace-bound Transformer wrapper stay aligned on {} trace steps, prompt_token_count={}, target_token_count={}, and output_count={}",
                row.fixture_trace_step_count,
                row.prompt_token_count,
                row.target_token_count,
                row.output_count
            )
        } else if !include_forward_evidence {
            format!(
                "research-only behavior parity stays exact on {} trace steps, prompt_token_count={}, target_token_count={}, and output_count={} without rebinding canonical forward-artifact identity checks",
                row.fixture_trace_step_count,
                row.prompt_token_count,
                row.target_token_count,
                row.output_count
            )
        } else {
            format!(
                "fixture reference-linear execution and the trained trace-bound Transformer wrapper keep exact full-trace and terminal-state parity on {} trace steps and {} target tokens; forward evidence remains a later bounded-window concern for this long-horizon case",
                row.fixture_trace_step_count,
                row.target_token_count
            )
        }
    } else {
        failure_detail(&row)
    };
    row
}

fn trace_shape_parity(
    roundtrip: &TassadarArticleTransformerTraceDomainRoundtrip,
    trace_diff_report: &TassadarTraceDiffReport,
) -> bool {
    roundtrip.binding.prompt_trace_boundary_supported
        && roundtrip.binding.halt_boundary_supported
        && roundtrip.batch.source_shape == vec![1, roundtrip.batch.prompt_token_count]
        && roundtrip.batch.target_shape == vec![1, roundtrip.batch.target_token_count]
        && roundtrip.batch.halt_marker.is_some()
        && roundtrip.prompt_boundary_preserved
        && roundtrip.halt_marker_preserved
        && roundtrip.roundtrip_exact
        && trace_diff_report.exact_match
}

fn execution_output_parity(expected: &TassadarExecution, actual: &TassadarExecution) -> bool {
    expected.outputs == actual.outputs
        && expected.final_locals == actual.final_locals
        && expected.final_memory == actual.final_memory
        && expected.final_stack == actual.final_stack
        && expected.halt_reason == actual.halt_reason
}

fn forward_binding_parity(
    batch: &TassadarArticleTransformerTraceDomainBatch,
    evidence: &TassadarArticleTransformerForwardPassEvidenceBundle,
    expected_model_artifact_binding: &TassadarArticleTransformerModelArtifactBinding,
    expected_environment_refs: &[String],
) -> bool {
    evidence.model_artifact == *expected_model_artifact_binding
        && evidence.run_config.source_shape == batch.source_shape
        && evidence.run_config.source_token_ids == batch.source_token_ids
        && evidence.run_config.target_shape == batch.target_shape
        && evidence.run_config.target_token_ids == batch.target_token_ids
        && evidence.run_config.environment_refs == expected_environment_refs
        && evidence.trace_artifact.predicted_token_ids.len() == batch.target_token_count
        && evidence.decode_receipt.predicted_token_ids.len() == batch.target_token_count
}

fn within_transformer_context_window(
    transformer_model: &TassadarArticleTransformer,
    batch: &TassadarArticleTransformerTraceDomainBatch,
) -> bool {
    batch.source_token_ids.len() <= transformer_model.descriptor().config.max_source_positions
        && batch.target_token_ids.len()
            <= transformer_model.descriptor().config.max_target_positions
}

fn failure_detail(row: &TassadarArticleFixtureTransformerParityCaseRow) -> String {
    let mut reasons = Vec::new();
    if !row.trace_shape_parity {
        reasons.push(format!(
            "trace parity drifted with first_divergence_step_index={:?}",
            row.trace_diff_report.first_divergence_step_index
        ));
    }
    if !row.output_parity {
        reasons.push(String::from("final outputs or terminal state drifted"));
    }
    if !row.behavior_parity {
        reasons.push(String::from("behavior digest drifted"));
    }
    if row.within_transformer_context_window && !row.forward_binding_parity {
        reasons.push(String::from(
            "forward evidence no longer matches the trace batch or committed model artifact binding",
        ));
    }
    if row.within_transformer_context_window && !row.forward_replay_stable {
        reasons.push(String::from("forward replay receipt is not deterministic"));
    }
    if reasons.is_empty() {
        String::from("fixture-to-Transformer parity failed without a typed mismatch reason")
    } else {
        format!(
            "fixture-to-Transformer parity failed: {}",
            reasons.join("; ")
        )
    }
}

fn empty_trace_diff_report() -> TassadarTraceDiffReport {
    TassadarTraceDiffReport::from_steps(&[], &[])
}

fn expected_forward_environment_refs() -> Vec<String> {
    let mut refs = vec![
        String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_ENVIRONMENT_REF),
        String::from(TASSADAR_ARTICLE_CLASS_BENCHMARK_REPORT_REF),
        String::from(TASSADAR_ARTICLE_TRANSFORMER_WEIGHT_LINEAGE_CONTRACT_REF),
    ];
    refs.sort();
    refs.dedup();
    refs
}

pub fn tassadar_article_fixture_transformer_parity_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF)
}

pub fn write_tassadar_article_fixture_transformer_parity_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFixtureTransformerParityReport,
    TassadarArticleFixtureTransformerParityError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFixtureTransformerParityError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_fixture_transformer_parity_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFixtureTransformerParityError::Write {
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
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleFixtureTransformerParityError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleFixtureTransformerParityError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFixtureTransformerParityError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_fixture_transformer_parity_report, execution_output_parity,
        expected_forward_environment_refs, forward_binding_parity, read_repo_json, repo_root,
        tassadar_article_fixture_transformer_parity_report_path, trace_shape_parity,
        write_tassadar_article_fixture_transformer_parity_report,
        TassadarArticleFixtureTransformerParityReport,
        TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
    };
    use psionic_core::Shape;
    use psionic_models::TassadarArticleTransformer;
    use psionic_runtime::{
        tassadar_article_class_corpus, TassadarFixtureRunner, TassadarTraceDiffReport,
    };
    use psionic_transformer::TransformerExecutionMode;

    #[test]
    fn article_fixture_transformer_parity_report_certifies_replacement_without_final_green() {
        let report = build_tassadar_article_fixture_transformer_parity_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.transformer_model_matches_lineage_contract);
        assert!(report.all_declared_cases_present);
        assert!(report.all_cases_pass);
        assert!(report.replacement_certified);
        assert!(report.replacement_publication_allowed);
        assert!(report.article_equivalence_green);
        assert_eq!(report.supported_case_count, 13);
        assert_eq!(report.routeable_case_count, 13);
        assert_eq!(report.exact_trace_case_count, 13);
        assert_eq!(report.exact_output_case_count, 13);
        assert_eq!(report.context_window_fit_case_count, 4);
        assert_eq!(report.forward_binding_case_count, 4);
        assert!(report.mismatch_case_ids.is_empty());
        assert_eq!(
            report
                .acceptance_gate_tie
                .blocked_issue_ids
                .first()
                .map(String::as_str),
            None
        );
    }

    #[test]
    fn article_fixture_transformer_parity_report_matches_committed_truth() {
        let generated = build_tassadar_article_fixture_transformer_parity_report().expect("report");
        let committed: TassadarArticleFixtureTransformerParityReport = read_repo_json(
            TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF,
            "article_fixture_transformer_parity_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_fixture_transformer_parity_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_fixture_transformer_parity_report.json");
        let written = write_tassadar_article_fixture_transformer_parity_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleFixtureTransformerParityReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_fixture_transformer_parity_report_path(),
            repo_root().join(TASSADAR_ARTICLE_FIXTURE_TRANSFORMER_PARITY_REPORT_REF)
        );
    }

    #[test]
    fn execution_output_parity_detects_drifted_outputs() {
        let case = tassadar_article_class_corpus()
            .into_iter()
            .next()
            .expect("article case");
        let execution = TassadarFixtureRunner::for_program(&case.program)
            .expect("runner")
            .execute(&case.program)
            .expect("execution");
        let mut drifted = execution.clone();
        drifted.outputs.push(999);

        assert!(!execution_output_parity(&execution, &drifted));
    }

    #[test]
    fn trace_and_forward_binding_helpers_detect_drift() {
        let case = tassadar_article_class_corpus()
            .into_iter()
            .next()
            .expect("article case");
        let execution = TassadarFixtureRunner::for_program(&case.program)
            .expect("runner")
            .execute(&case.program)
            .expect("execution");
        let model = TassadarArticleTransformer::trained_trace_domain_reference().expect("model");
        let roundtrip = model
            .roundtrip_article_trace_domain(&case.program, &execution)
            .expect("roundtrip");
        let trace_diff = TassadarTraceDiffReport::from_executions(&execution, &execution);
        assert!(trace_shape_parity(&roundtrip, &trace_diff));

        let evidence = model
            .forward_with_runtime_evidence(
                "fixture-parity-run",
                "fixture-parity-request",
                "psionic.article_transformer.fixture_parity",
                expected_forward_environment_refs(),
                Shape::new(roundtrip.batch.source_shape.clone()),
                &roundtrip.batch.source_token_ids,
                Shape::new(roundtrip.batch.target_shape.clone()),
                &roundtrip.batch.target_token_ids,
                TransformerExecutionMode::Eval,
                None,
            )
            .expect("evidence");
        assert!(forward_binding_parity(
            &roundtrip.batch,
            &evidence,
            &model.model_artifact_binding(),
            expected_forward_environment_refs().as_slice(),
        ));

        let mut drifted_roundtrip = roundtrip.clone();
        drifted_roundtrip.batch.source_shape[1] += 1;
        assert!(!trace_shape_parity(&drifted_roundtrip, &trace_diff));

        let mut drifted_evidence = evidence.clone();
        drifted_evidence.run_config.target_shape[1] += 1;
        assert!(!forward_binding_parity(
            &roundtrip.batch,
            &drifted_evidence,
            &model.model_artifact_binding(),
            expected_forward_environment_refs().as_slice(),
        ));
    }
}
