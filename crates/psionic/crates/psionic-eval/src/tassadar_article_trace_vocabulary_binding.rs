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
    TassadarArticleTransformer, TassadarArticleTransformerError,
    TassadarArticleTransformerTraceDomainBinding, TassadarArticleTransformerTraceDomainRoundtrip,
    TassadarTraceTokenizer, TokenizerBoundary,
};
use psionic_runtime::{
    tassadar_article_class_corpus, TassadarCpuReferenceRunner, TassadarExecutionRefusal,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_canonical_transformer_stack_boundary_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarCanonicalTransformerStackBoundaryReport,
    TassadarCanonicalTransformerStackBoundaryReportError,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
};

pub const TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_trace_vocabulary_binding_report.json";

const TIED_REQUIREMENT_ID: &str = "TAS-167";
const BOUNDARY_DOC_REF: &str = "docs/TASSADAR_ARTICLE_TRANSFORMER_STACK_BOUNDARY.md";
const EXECUTOR_TRANSFORMER_REF: &str = "crates/psionic-models/src/tassadar_executor_transformer.rs";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleTraceVocabularyValidationKind {
    TokenizerVocabularyCompatibility,
    PromptTraceBoundarySplit,
    StateChannelBinding,
    TypedTraceRoundtrip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceVocabularyCaseRow {
    pub case_id: String,
    pub validation_kind: TassadarArticleTraceVocabularyValidationKind,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceVocabularyBoundaryReview {
    pub boundary_doc_ref: String,
    pub tokenizer_module_ref: String,
    pub model_module_ref: String,
    pub runtime_trace_schema_module_ref: String,
    pub executor_transformer_ref: String,
    pub boundary_doc_names_runtime_owned_trace_schema: bool,
    pub boundary_doc_names_tokenizer_binding: bool,
    pub model_defines_trace_bound_reference: bool,
    pub model_defines_trace_binding_helpers: bool,
    pub runtime_schema_module_exists: bool,
    pub legacy_executor_transformer_remains_noncanonical: bool,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceVocabularyAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub canonical_boundary_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleTraceVocabularyBindingReport {
    pub schema_version: u16,
    pub report_id: String,
    pub acceptance_gate_tie: TassadarArticleTraceVocabularyAcceptanceGateTie,
    pub canonical_boundary_report_ref: String,
    pub canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    pub trace_domain_binding: TassadarArticleTransformerTraceDomainBinding,
    pub roundtrip: TassadarArticleTransformerTraceDomainRoundtrip,
    pub case_rows: Vec<TassadarArticleTraceVocabularyCaseRow>,
    pub boundary_review: TassadarArticleTraceVocabularyBoundaryReview,
    pub all_required_cases_present: bool,
    pub all_cases_pass: bool,
    pub article_trace_vocabulary_binding_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleTraceVocabularyBindingReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    CanonicalBoundary(#[from] TassadarCanonicalTransformerStackBoundaryReportError),
    #[error(transparent)]
    Model(#[from] TassadarArticleTransformerError),
    #[error(transparent)]
    RuntimeExecution(#[from] TassadarExecutionRefusal),
    #[error("the article-class corpus is empty")]
    EmptyArticleCorpus,
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

pub fn build_tassadar_article_trace_vocabulary_binding_report() -> Result<
    TassadarArticleTraceVocabularyBindingReport,
    TassadarArticleTraceVocabularyBindingReportError,
> {
    let acceptance_gate_report = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let canonical_boundary_report = build_tassadar_canonical_transformer_stack_boundary_report()?;
    let model = TassadarArticleTransformer::article_trace_domain_reference()?;
    let trace_domain_binding = model.trace_domain_binding();
    let case = tassadar_article_class_corpus()
        .into_iter()
        .next()
        .ok_or(TassadarArticleTraceVocabularyBindingReportError::EmptyArticleCorpus)?;
    let execution =
        TassadarCpuReferenceRunner::for_program(&case.program)?.execute(&case.program)?;
    let roundtrip = model.roundtrip_article_trace_domain(&case.program, &execution)?;
    let case_rows = case_rows(&trace_domain_binding, &roundtrip);
    let boundary_review = boundary_review()?;
    Ok(build_report_from_inputs(
        acceptance_gate_report,
        canonical_boundary_report,
        trace_domain_binding,
        roundtrip,
        case_rows,
        boundary_review,
    ))
}

fn build_report_from_inputs(
    acceptance_gate_report: TassadarArticleEquivalenceAcceptanceGateReport,
    canonical_boundary_report: TassadarCanonicalTransformerStackBoundaryReport,
    trace_domain_binding: TassadarArticleTransformerTraceDomainBinding,
    roundtrip: TassadarArticleTransformerTraceDomainRoundtrip,
    case_rows: Vec<TassadarArticleTraceVocabularyCaseRow>,
    boundary_review: TassadarArticleTraceVocabularyBoundaryReview,
) -> TassadarArticleTraceVocabularyBindingReport {
    let acceptance_gate_tie = TassadarArticleTraceVocabularyAcceptanceGateTie {
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
    let article_trace_vocabulary_binding_green = acceptance_gate_tie.tied_requirement_satisfied
        && canonical_boundary_report.boundary_contract_green
        && trace_domain_binding.source_vocab_compatible
        && trace_domain_binding.target_vocab_compatible
        && trace_domain_binding.prompt_trace_boundary_supported
        && trace_domain_binding.halt_boundary_supported
        && trace_domain_binding.all_required_channels_bound
        && roundtrip.roundtrip_exact
        && all_required_cases_present
        && all_cases_pass
        && boundary_review.passed;
    let article_equivalence_green =
        article_trace_vocabulary_binding_green && acceptance_gate_report.article_equivalence_green;

    let mut report = TassadarArticleTraceVocabularyBindingReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_trace_vocabulary_binding.report.v1"),
        acceptance_gate_tie,
        canonical_boundary_report_ref: String::from(
            TASSADAR_CANONICAL_TRANSFORMER_STACK_BOUNDARY_REPORT_REF,
        ),
        canonical_boundary_report,
        trace_domain_binding,
        roundtrip,
        case_rows,
        boundary_review,
        all_required_cases_present,
        all_cases_pass,
        article_trace_vocabulary_binding_green,
        article_equivalence_green,
        claim_boundary: String::from(
            "this report closes the owned-route trace vocabulary and channel binding only. It proves that the canonical article wrapper can consume one runtime-owned prompt/trace schema, split prompt tokens from the append-only trace suffix, bind stack/local/memory channels directly to that schema, and roundtrip one article-class case through the shared tokenizer without drift. It does not claim representation invariance, artifact-backed weight identity, reference-linear exactness, fast-route promotion, benchmark parity, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article trace vocabulary binding now records channel_binding_rows={}, case_rows={}, source_vocab_compatible={}, target_vocab_compatible={}, prompt_trace_boundary_supported={}, halt_boundary_supported={}, all_required_channels_bound={}, roundtrip_exact={}, article_trace_vocabulary_binding_green={}, and article_equivalence_green={}.",
        report.trace_domain_binding.channel_binding_rows.len(),
        report.case_rows.len(),
        report.trace_domain_binding.source_vocab_compatible,
        report.trace_domain_binding.target_vocab_compatible,
        report.trace_domain_binding.prompt_trace_boundary_supported,
        report.trace_domain_binding.halt_boundary_supported,
        report.trace_domain_binding.all_required_channels_bound,
        report.roundtrip.roundtrip_exact,
        report.article_trace_vocabulary_binding_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_trace_vocabulary_binding_report|",
        &report,
    );
    report
}

fn required_validation_kinds() -> BTreeSet<TassadarArticleTraceVocabularyValidationKind> {
    BTreeSet::from([
        TassadarArticleTraceVocabularyValidationKind::TokenizerVocabularyCompatibility,
        TassadarArticleTraceVocabularyValidationKind::PromptTraceBoundarySplit,
        TassadarArticleTraceVocabularyValidationKind::StateChannelBinding,
        TassadarArticleTraceVocabularyValidationKind::TypedTraceRoundtrip,
    ])
}

fn case_rows(
    trace_domain_binding: &TassadarArticleTransformerTraceDomainBinding,
    roundtrip: &TassadarArticleTransformerTraceDomainRoundtrip,
) -> Vec<TassadarArticleTraceVocabularyCaseRow> {
    vec![
        tokenizer_vocabulary_compatibility_case(trace_domain_binding),
        prompt_trace_boundary_split_case(roundtrip),
        state_channel_binding_case(trace_domain_binding),
        typed_trace_roundtrip_case(roundtrip),
    ]
}

fn tokenizer_vocabulary_compatibility_case(
    trace_domain_binding: &TassadarArticleTransformerTraceDomainBinding,
) -> TassadarArticleTraceVocabularyCaseRow {
    let passed = trace_domain_binding.source_vocab_compatible
        && trace_domain_binding.target_vocab_compatible
        && trace_domain_binding.model_source_vocab_size
            == trace_domain_binding.tokenizer_vocab_size
        && trace_domain_binding.model_target_vocab_size
            == trace_domain_binding.tokenizer_vocab_size;
    let detail = if passed {
        format!(
            "the trace-bound article wrapper matches the tokenizer vocabulary exactly at vocab_size={}",
            trace_domain_binding.tokenizer_vocab_size
        )
    } else {
        format!(
            "model vocab sizes source={} target={} do not match tokenizer_vocab_size={}",
            trace_domain_binding.model_source_vocab_size,
            trace_domain_binding.model_target_vocab_size,
            trace_domain_binding.tokenizer_vocab_size
        )
    };
    TassadarArticleTraceVocabularyCaseRow {
        case_id: String::from("tokenizer_vocabulary_compatibility"),
        validation_kind:
            TassadarArticleTraceVocabularyValidationKind::TokenizerVocabularyCompatibility,
        passed,
        detail,
    }
}

fn prompt_trace_boundary_split_case(
    roundtrip: &TassadarArticleTransformerTraceDomainRoundtrip,
) -> TassadarArticleTraceVocabularyCaseRow {
    let passed = roundtrip.binding.prompt_trace_boundary_supported
        && roundtrip.binding.halt_boundary_supported
        && roundtrip.batch.source_shape == vec![1, roundtrip.batch.prompt_token_count]
        && roundtrip.batch.target_shape == vec![1, roundtrip.batch.target_token_count]
        && roundtrip.prompt_boundary_preserved
        && roundtrip.batch.halt_marker.is_some();
    let detail = if passed {
        format!(
            "the owned route keeps the prompt prefix at {} tokens, the append-only trace suffix at {} tokens, and an explicit halt marker {:?}",
            roundtrip.batch.prompt_token_count,
            roundtrip.batch.target_token_count,
            roundtrip.batch.halt_marker
        )
    } else {
        String::from(
            "the prompt/trace boundary or halt marker drifted from the runtime-owned article trace contract",
        )
    };
    TassadarArticleTraceVocabularyCaseRow {
        case_id: String::from("prompt_trace_boundary_split"),
        validation_kind: TassadarArticleTraceVocabularyValidationKind::PromptTraceBoundarySplit,
        passed,
        detail,
    }
}

fn state_channel_binding_case(
    trace_domain_binding: &TassadarArticleTransformerTraceDomainBinding,
) -> TassadarArticleTraceVocabularyCaseRow {
    let tokenizer = TassadarTraceTokenizer::new();
    let passed = trace_domain_binding.all_required_channels_bound
        && trace_domain_binding.channel_binding_rows.len()
            == trace_domain_binding.trace_schema.channel_rows.len()
        && trace_domain_binding.channel_binding_rows.iter().all(|row| {
            row.bound
                && row.token_forms.iter().all(|token| {
                    tokenizer
                        .vocabulary()
                        .tokens()
                        .iter()
                        .any(|value| value == token)
                })
        });
    let detail = if passed {
        format!(
            "all {} runtime-owned channel rows bind to shared tokenizer forms, including stack/local/memory channels and terminal halt markers",
            trace_domain_binding.channel_binding_rows.len()
        )
    } else {
        String::from(
            "one or more runtime-owned channel rows no longer map cleanly onto the shared tokenizer forms",
        )
    };
    TassadarArticleTraceVocabularyCaseRow {
        case_id: String::from("state_channel_binding"),
        validation_kind: TassadarArticleTraceVocabularyValidationKind::StateChannelBinding,
        passed,
        detail,
    }
}

fn typed_trace_roundtrip_case(
    roundtrip: &TassadarArticleTransformerTraceDomainRoundtrip,
) -> TassadarArticleTraceVocabularyCaseRow {
    let passed = roundtrip.prompt_boundary_preserved
        && roundtrip.halt_marker_preserved
        && roundtrip.roundtrip_exact;
    let detail = if passed {
        format!(
            "the trace-bound article wrapper roundtrips one article-class case exactly with prompt_token_count={}, target_token_count={}, and sequence_digest={}",
            roundtrip.batch.prompt_token_count,
            roundtrip.batch.target_token_count,
            roundtrip.batch.sequence_digest
        )
    } else {
        String::from(
            "the typed program/execution reconstruction drifted from the original article-class execution",
        )
    };
    TassadarArticleTraceVocabularyCaseRow {
        case_id: String::from("typed_trace_roundtrip"),
        validation_kind: TassadarArticleTraceVocabularyValidationKind::TypedTraceRoundtrip,
        passed,
        detail,
    }
}

fn boundary_review() -> Result<
    TassadarArticleTraceVocabularyBoundaryReview,
    TassadarArticleTraceVocabularyBindingReportError,
> {
    let boundary_doc = read_repo_text(BOUNDARY_DOC_REF)?;
    let tokenizer_module = read_repo_text(TassadarArticleTransformer::TOKENIZER_MODULE_REF)?;
    let model_module = read_repo_text(TassadarArticleTransformer::MODEL_MODULE_REF)?;
    let runtime_trace_schema = read_repo_text(TassadarArticleTransformer::TRACE_SCHEMA_MODULE_REF)?;
    let executor_transformer = read_repo_text(EXECUTOR_TRANSFORMER_REF)?;
    let boundary_doc_names_runtime_owned_trace_schema = boundary_doc
        .contains("runtime-owned machine-step schema")
        && boundary_doc.contains("`crates/psionic-runtime/src/tassadar_article_trace_schema.rs`");
    let boundary_doc_names_tokenizer_binding = boundary_doc
        .contains("`crates/psionic-models/src/tassadar_sequence.rs`")
        && boundary_doc.contains("trace tokenizer");
    let model_defines_trace_bound_reference = contains_all(
        &model_module,
        &[
            "TRACE_BOUND_MODEL_ID",
            "article_trace_domain_reference",
            "trace_domain_reference_config",
        ],
    );
    let model_defines_trace_binding_helpers = contains_all(
        &model_module,
        &[
            "trace_domain_binding",
            "encode_article_trace_domain",
            "roundtrip_article_trace_domain",
        ],
    ) && contains_all(
        &tokenizer_module,
        &[
            "decode_article_trace_domain",
            "compose_prompt_and_target_sequence",
            "pub struct TassadarDecodedArticleTraceDomain",
        ],
    );
    let runtime_schema_module_exists = contains_all(
        &runtime_trace_schema,
        &[
            "pub struct TassadarArticleTraceMachineStepSchema",
            "pub struct TassadarArticleTraceBoundaryContract",
            "pub enum TassadarArticleTraceChannelKind",
            "pub fn tassadar_article_trace_machine_step_schema()",
        ],
    );
    let legacy_executor_transformer_remains_noncanonical = executor_transformer
        .contains("pub struct TassadarExecutorTransformer")
        && boundary_doc.contains("separate research and comparison lane");
    let passed = boundary_doc_names_runtime_owned_trace_schema
        && boundary_doc_names_tokenizer_binding
        && model_defines_trace_bound_reference
        && model_defines_trace_binding_helpers
        && runtime_schema_module_exists
        && legacy_executor_transformer_remains_noncanonical;
    let detail = if passed {
        String::from(
            "the boundary doc, tokenizer, canonical article wrapper, and runtime trace-schema module all agree on the owned-route split: runtime owns the machine-step schema, `psionic-models` owns the tokenizer and wrapper binding, and the older executor-transformer remains a non-canonical comparison lane",
        )
    } else {
        String::from(
            "the trace vocabulary boundary review drifted: one or more of the runtime-owned schema markers, tokenizer markers, canonical wrapper helpers, or non-canonical legacy-lane markers is missing",
        )
    };
    Ok(TassadarArticleTraceVocabularyBoundaryReview {
        boundary_doc_ref: String::from(BOUNDARY_DOC_REF),
        tokenizer_module_ref: String::from(TassadarArticleTransformer::TOKENIZER_MODULE_REF),
        model_module_ref: String::from(TassadarArticleTransformer::MODEL_MODULE_REF),
        runtime_trace_schema_module_ref: String::from(
            TassadarArticleTransformer::TRACE_SCHEMA_MODULE_REF,
        ),
        executor_transformer_ref: String::from(EXECUTOR_TRANSFORMER_REF),
        boundary_doc_names_runtime_owned_trace_schema,
        boundary_doc_names_tokenizer_binding,
        model_defines_trace_bound_reference,
        model_defines_trace_binding_helpers,
        runtime_schema_module_exists,
        legacy_executor_transformer_remains_noncanonical,
        passed,
        detail,
    })
}

pub fn tassadar_article_trace_vocabulary_binding_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF)
}

pub fn write_tassadar_article_trace_vocabulary_binding_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleTraceVocabularyBindingReport,
    TassadarArticleTraceVocabularyBindingReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleTraceVocabularyBindingReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_trace_vocabulary_binding_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleTraceVocabularyBindingReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn contains_all(value: &str, required_fragments: &[&str]) -> bool {
    required_fragments
        .iter()
        .all(|fragment| value.contains(fragment))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-eval should live under <repo>/crates/psionic-eval")
        .to_path_buf()
}

fn read_repo_text(
    relative_path: &str,
) -> Result<String, TassadarArticleTraceVocabularyBindingReportError> {
    let path = repo_root().join(relative_path);
    fs::read_to_string(&path).map_err(|error| {
        TassadarArticleTraceVocabularyBindingReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })
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
) -> Result<T, TassadarArticleTraceVocabularyBindingReportError> {
    let path = path.as_ref();
    let bytes =
        fs::read(path).map_err(
            |error| TassadarArticleTraceVocabularyBindingReportError::Read {
                path: path.display().to_string(),
                error,
            },
        )?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleTraceVocabularyBindingReportError::Decode {
            path: path.display().to_string(),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use psionic_models::TassadarArticleTransformer;

    use super::{
        boundary_review, build_report_from_inputs,
        build_tassadar_article_trace_vocabulary_binding_report, read_json,
        tassadar_article_trace_vocabulary_binding_report_path,
        write_tassadar_article_trace_vocabulary_binding_report,
        TassadarArticleTraceVocabularyBindingReport,
        TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF,
    };
    use crate::{
        build_tassadar_article_equivalence_acceptance_gate_report,
        build_tassadar_canonical_transformer_stack_boundary_report,
    };

    #[test]
    fn article_trace_vocabulary_binding_tracks_green_binding_without_final_green() {
        let report =
            build_tassadar_article_trace_vocabulary_binding_report().expect("binding report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(
            report.acceptance_gate_tie.acceptance_status,
            crate::TassadarArticleEquivalenceAcceptanceStatus::Green
        );
        assert_eq!(report.trace_domain_binding.channel_binding_rows.len(), 14);
        assert!(report.trace_domain_binding.source_vocab_compatible);
        assert!(report.trace_domain_binding.target_vocab_compatible);
        assert!(report.trace_domain_binding.prompt_trace_boundary_supported);
        assert!(report.trace_domain_binding.halt_boundary_supported);
        assert!(report.trace_domain_binding.all_required_channels_bound);
        assert!(report.roundtrip.roundtrip_exact);
        assert!(report.boundary_review.passed);
        assert!(report.article_trace_vocabulary_binding_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn failed_roundtrip_keeps_article_trace_vocabulary_binding_red() {
        let acceptance_gate =
            build_tassadar_article_equivalence_acceptance_gate_report().expect("gate");
        let canonical_boundary =
            build_tassadar_canonical_transformer_stack_boundary_report().expect("boundary");
        let model = TassadarArticleTransformer::article_trace_domain_reference().expect("model");
        let binding = model.trace_domain_binding();
        let case = psionic_runtime::tassadar_article_class_corpus()
            .into_iter()
            .next()
            .expect("article corpus");
        let execution = psionic_runtime::TassadarCpuReferenceRunner::for_program(&case.program)
            .expect("runner")
            .execute(&case.program)
            .expect("execution");
        let mut roundtrip = model
            .roundtrip_article_trace_domain(&case.program, &execution)
            .expect("roundtrip");
        roundtrip.roundtrip_exact = false;
        let case_rows = super::case_rows(&binding, &roundtrip);
        let review = boundary_review().expect("review");
        let report = build_report_from_inputs(
            acceptance_gate,
            canonical_boundary,
            binding,
            roundtrip,
            case_rows,
            review,
        );

        assert!(!report.article_trace_vocabulary_binding_green);
        assert!(!report.article_equivalence_green);
    }

    #[test]
    fn article_trace_vocabulary_binding_matches_committed_truth() {
        let generated =
            build_tassadar_article_trace_vocabulary_binding_report().expect("binding report");
        let committed: TassadarArticleTraceVocabularyBindingReport =
            read_json(tassadar_article_trace_vocabulary_binding_report_path())
                .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_trace_vocabulary_binding_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_trace_vocabulary_binding_report.json");
        let written = write_tassadar_article_trace_vocabulary_binding_report(&output_path)
            .expect("written report");

        assert_eq!(
            written,
            read_json(tassadar_article_trace_vocabulary_binding_report_path())
                .expect("committed report")
        );
        assert_eq!(
            output_path.file_name().and_then(|value| value.to_str()),
            Some("tassadar_article_trace_vocabulary_binding_report.json")
        );
        assert_eq!(
            TASSADAR_ARTICLE_TRACE_VOCABULARY_BINDING_REPORT_REF,
            "fixtures/tassadar/reports/tassadar_article_trace_vocabulary_binding_report.json"
        );
    }
}
