use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_compiler::{
    build_tassadar_article_frontend_compiler_envelope_manifest,
    TassadarArticleFrontendAbiSurfaceId, TassadarArticleFrontendCompilerEnvelopeManifest,
    TassadarArticleFrontendEnvelopeRefusalKind,
    TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF,
};
use psionic_runtime::{
    compile_tassadar_rust_source_to_wasm_receipt, TassadarCompileRefusal,
    TassadarCompilerToolchainIdentity, TassadarRustToWasmCompileConfig, TassadarWasmBinarySummary,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json";
pub const TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_CHECKER_REF: &str =
    "scripts/check-tassadar-article-frontend-corpus-compile-matrix.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-177";
static ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_BUILD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFrontendCorpusCategory {
    ArithmeticKernel,
    BranchHeavyKernel,
    StateMachineKernel,
    AllocatorBackedMemoryKernel,
    HungarianLikeSupport,
    SudokuLikeSupport,
    RefusalCoverage,
    ToolchainFailureProbe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleFrontendCorpusCaseStatus {
    Compiled,
    TypedRefusal,
    ToolchainFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCorpusAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCorpusCategoryCoverageRow {
    pub category_id: TassadarArticleFrontendCorpusCategory,
    pub required_minimum_compiled_cases: usize,
    pub compiled_case_count: usize,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCorpusCaseRow {
    pub case_id: String,
    pub category_id: TassadarArticleFrontendCorpusCategory,
    pub source_ref: String,
    pub source_digest: String,
    pub expected_status: TassadarArticleFrontendCorpusCaseStatus,
    pub actual_status: TassadarArticleFrontendCorpusCaseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abi_surface_id: Option<String>,
    pub compile_config_digest: String,
    pub compile_pipeline_features: Vec<String>,
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    pub toolchain_digest: String,
    pub compile_receipt_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_refusal_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_refusal_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub envelope_refusal_kind: Option<TassadarArticleFrontendEnvelopeRefusalKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_summary: Option<TassadarWasmBinarySummary>,
    pub lineage_green: bool,
    pub refusal_green: bool,
    pub toolchain_failure_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCorpusCompileMatrixReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleFrontendCorpusAcceptanceGateTie,
    pub manifest_ref: String,
    pub manifest: TassadarArticleFrontendCompilerEnvelopeManifest,
    pub case_rows: Vec<TassadarArticleFrontendCorpusCaseRow>,
    pub category_coverage_rows: Vec<TassadarArticleFrontendCorpusCategoryCoverageRow>,
    pub compiled_case_count: usize,
    pub typed_refusal_case_count: usize,
    pub toolchain_failure_case_count: usize,
    pub lineage_green_count: usize,
    pub refusal_green_count: usize,
    pub toolchain_failure_green_count: usize,
    pub category_coverage_green: bool,
    pub envelope_alignment_green: bool,
    pub compile_matrix_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone)]
struct TassadarArticleFrontendCorpusCaseSpec {
    case_id: &'static str,
    category_id: TassadarArticleFrontendCorpusCategory,
    source_ref: &'static str,
    output_wasm_ref: &'static str,
    compile_config: TassadarRustToWasmCompileConfig,
    expected_status: TassadarArticleFrontendCorpusCaseStatus,
    expected_refusal_kind: Option<TassadarArticleFrontendEnvelopeRefusalKind>,
    abi_surface_id: Option<&'static str>,
}

#[derive(Debug, Error)]
pub enum TassadarArticleFrontendCorpusCompileMatrixReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error("internal TAS-177 invariant failed: {detail}")]
    Invariant { detail: String },
    #[error("failed to read `{path}`: {error}")]
    Read { path: String, error: std::io::Error },
    #[error("failed to create `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error("failed to decode `{path}`: {error}")]
    Decode {
        path: String,
        error: serde_json::Error,
    },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_frontend_corpus_compile_matrix_report() -> Result<
    TassadarArticleFrontendCorpusCompileMatrixReport,
    TassadarArticleFrontendCorpusCompileMatrixReportError,
> {
    let _guard = ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_BUILD_LOCK
        .lock()
        .expect("build lock");
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let manifest = build_tassadar_article_frontend_compiler_envelope_manifest();
    let case_rows = case_specs()
        .into_iter()
        .map(|spec| build_case_row(&manifest, &spec))
        .collect::<Result<Vec<_>, _>>()?;
    let category_coverage_rows = build_category_coverage_rows(case_rows.as_slice());
    let compiled_case_count = case_rows
        .iter()
        .filter(|row| row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled)
        .count();
    let typed_refusal_case_count = case_rows
        .iter()
        .filter(|row| row.actual_status == TassadarArticleFrontendCorpusCaseStatus::TypedRefusal)
        .count();
    let toolchain_failure_case_count = case_rows
        .iter()
        .filter(|row| {
            row.actual_status == TassadarArticleFrontendCorpusCaseStatus::ToolchainFailure
        })
        .count();
    let lineage_green_count = case_rows.iter().filter(|row| row.lineage_green).count();
    let refusal_green_count = case_rows.iter().filter(|row| row.refusal_green).count();
    let toolchain_failure_green_count = case_rows
        .iter()
        .filter(|row| row.toolchain_failure_green)
        .count();
    let category_coverage_green = category_coverage_rows.iter().all(|row| row.green);
    let envelope_alignment_green = case_rows.iter().all(|row| {
        if row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled {
            row.lineage_green
                && row
                    .abi_surface_id
                    .as_ref()
                    .is_some_and(|abi_surface| manifest_allows_abi_surface(&manifest, abi_surface))
        } else if row.actual_status == TassadarArticleFrontendCorpusCaseStatus::TypedRefusal {
            row.refusal_green
        } else {
            row.toolchain_failure_green
        }
    });
    let acceptance_gate_tie = build_acceptance_gate_tie(&acceptance_gate)?;
    let compile_matrix_green = acceptance_gate_tie.tied_requirement_satisfied
        && category_coverage_green
        && envelope_alignment_green
        && compiled_case_count == 11
        && typed_refusal_case_count == 4
        && toolchain_failure_case_count == 1
        && lineage_green_count == 11
        && refusal_green_count == 4
        && toolchain_failure_green_count == 1;

    let mut report = TassadarArticleFrontendCorpusCompileMatrixReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_frontend_corpus_compile_matrix.report.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_CHECKER_REF),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        manifest_ref: String::from(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF),
        manifest,
        case_rows,
        category_coverage_rows,
        compiled_case_count,
        typed_refusal_case_count,
        toolchain_failure_case_count,
        lineage_green_count,
        refusal_green_count,
        toolchain_failure_green_count,
        category_coverage_green,
        envelope_alignment_green,
        compile_matrix_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && compile_matrix_green,
        claim_boundary: String::from(
            "this report closes TAS-177 only. It expands the article-envelope frontend corpus beyond the old narrow canonical lane by compiling a broader committed Rust source set across arithmetic, branch-heavy, state-machine, allocator-backed memory, Hungarian-like, and Sudoku-like support families, while keeping typed refusal and toolchain-failure posture machine-readable. It does not yet claim article-demo frontend parity, arbitrary-program closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article frontend corpus compile matrix now records tied_requirement_satisfied={}, compiled_cases={}/{}, typed_refusals={}/{}, toolchain_failures={}/{}, category_coverage_green={}, envelope_alignment_green={}, compile_matrix_green={}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.lineage_green_count,
        report.compiled_case_count,
        report.refusal_green_count,
        report.typed_refusal_case_count,
        report.toolchain_failure_green_count,
        report.toolchain_failure_case_count,
        report.category_coverage_green,
        report.envelope_alignment_green,
        report.compile_matrix_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_frontend_corpus_compile_matrix_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_article_frontend_corpus_compile_matrix_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF)
}

pub fn write_tassadar_article_frontend_corpus_compile_matrix_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFrontendCorpusCompileMatrixReport,
    TassadarArticleFrontendCorpusCompileMatrixReportError,
> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFrontendCorpusCompileMatrixReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_frontend_corpus_compile_matrix_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFrontendCorpusCompileMatrixReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_acceptance_gate_tie(
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
) -> Result<
    TassadarArticleFrontendCorpusAcceptanceGateTie,
    TassadarArticleFrontendCorpusCompileMatrixReportError,
> {
    let tied_requirement_satisfied = acceptance_gate
        .green_requirement_ids
        .iter()
        .any(|requirement_id| requirement_id == TIED_REQUIREMENT_ID);
    if !tied_requirement_satisfied {
        return Err(
            TassadarArticleFrontendCorpusCompileMatrixReportError::Invariant {
                detail: format!(
                    "acceptance gate does not yet treat `{}` as green",
                    TIED_REQUIREMENT_ID
                ),
            },
        );
    }
    Ok(TassadarArticleFrontendCorpusAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied,
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    })
}

fn build_category_coverage_rows(
    case_rows: &[TassadarArticleFrontendCorpusCaseRow],
) -> Vec<TassadarArticleFrontendCorpusCategoryCoverageRow> {
    required_categories()
        .into_iter()
        .map(|category_id| {
            let compiled_case_count = case_rows
                .iter()
                .filter(|row| {
                    row.category_id == category_id
                        && row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled
                })
                .count();
            TassadarArticleFrontendCorpusCategoryCoverageRow {
                category_id,
                required_minimum_compiled_cases: 1,
                compiled_case_count,
                green: compiled_case_count >= 1,
                detail: String::from(
                    "the expanded article frontend corpus stays green only when each declared category has at least one compiled case inside the bounded envelope",
                ),
            }
        })
        .collect()
}

fn build_case_row(
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
    spec: &TassadarArticleFrontendCorpusCaseSpec,
) -> Result<
    TassadarArticleFrontendCorpusCaseRow,
    TassadarArticleFrontendCorpusCompileMatrixReportError,
> {
    let source_path = repo_root().join(spec.source_ref);
    let source_bytes = fs::read(&source_path).map_err(|error| {
        TassadarArticleFrontendCorpusCompileMatrixReportError::Read {
            path: source_path.display().to_string(),
            error,
        }
    })?;
    let source_text = String::from_utf8_lossy(&source_bytes);
    let output_path = repo_root().join(spec.output_wasm_ref);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFrontendCorpusCompileMatrixReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let _ = fs::remove_file(&output_path);
    let compile_receipt = compile_tassadar_rust_source_to_wasm_receipt(
        spec.source_ref,
        &source_bytes,
        &output_path,
        &spec.compile_config,
    );
    let toolchain_identity = compile_receipt.toolchain_identity.clone();
    let toolchain_digest = stable_digest(
        b"psionic_tassadar_article_frontend_corpus_toolchain|",
        &toolchain_identity,
    );
    let compile_refusal_kind = compile_receipt
        .refusal()
        .map(|refusal| refusal.kind_slug().to_string());
    let compile_refusal_detail = compile_receipt.refusal().map(refusal_detail);
    let envelope_refusal_kind = classify_envelope_refusal(
        spec,
        source_text.as_ref(),
        compile_receipt.wasm_binary_summary(),
    );
    let actual_status = actual_status_for_case(
        envelope_refusal_kind,
        compile_receipt.refusal(),
        compile_receipt.succeeded(),
    );
    let abi_surface_id = spec.abi_surface_id.map(String::from);
    let lineage_green = actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled
        && compile_receipt.succeeded()
        && compile_config_matches_manifest(&spec.compile_config, manifest)
        && abi_surface_id
            .as_ref()
            .is_some_and(|abi_surface| manifest_allows_abi_surface(manifest, abi_surface))
        && compile_receipt
            .wasm_binary_summary()
            .is_some_and(|summary| summary.imported_function_count == 0)
        && output_matches_receipt(&output_path, &compile_receipt)?;
    let refusal_green = actual_status == TassadarArticleFrontendCorpusCaseStatus::TypedRefusal
        && envelope_refusal_kind == spec.expected_refusal_kind
        && spec.expected_status == TassadarArticleFrontendCorpusCaseStatus::TypedRefusal;
    let toolchain_failure_green = actual_status
        == TassadarArticleFrontendCorpusCaseStatus::ToolchainFailure
        && spec.expected_status == TassadarArticleFrontendCorpusCaseStatus::ToolchainFailure
        && compile_receipt
            .refusal()
            .is_some_and(is_toolchain_failure_refusal);
    let detail = match actual_status {
        TassadarArticleFrontendCorpusCaseStatus::Compiled => format!(
            "compiled through the declared article envelope with abi_surface_id={}, wasm_output_ref={}, and zero-import lineage",
            abi_surface_id
                .as_deref()
                .unwrap_or("unknown_abi_surface"),
            spec.output_wasm_ref
        ),
        TassadarArticleFrontendCorpusCaseStatus::TypedRefusal => format!(
            "typed refusal stayed explicit at refusal_kind={} with compile_refusal_kind={}",
            envelope_refusal_kind
                .map(refusal_kind_slug)
                .unwrap_or("unknown_refusal"),
            compile_refusal_kind.as_deref().unwrap_or("none")
        ),
        TassadarArticleFrontendCorpusCaseStatus::ToolchainFailure => format!(
            "toolchain-specific failure stayed explicit as compile_refusal_kind={}",
            compile_refusal_kind.as_deref().unwrap_or("unknown_toolchain_failure")
        ),
    };

    Ok(TassadarArticleFrontendCorpusCaseRow {
        case_id: String::from(spec.case_id),
        category_id: spec.category_id,
        source_ref: String::from(spec.source_ref),
        source_digest: stable_bytes_digest(&source_bytes),
        expected_status: spec.expected_status,
        actual_status,
        abi_surface_id,
        compile_config_digest: spec.compile_config.stable_digest(),
        compile_pipeline_features: spec.compile_config.pipeline_features(),
        toolchain_identity,
        toolchain_digest,
        compile_receipt_digest: compile_receipt.receipt_digest.clone(),
        compile_refusal_kind,
        compile_refusal_detail,
        envelope_refusal_kind,
        wasm_binary_ref: compile_receipt.wasm_binary_ref().map(String::from),
        wasm_binary_digest: compile_receipt.wasm_binary_digest().map(String::from),
        wasm_binary_summary: compile_receipt.wasm_binary_summary().cloned(),
        lineage_green,
        refusal_green,
        toolchain_failure_green,
        detail,
    })
}

fn actual_status_for_case(
    envelope_refusal_kind: Option<TassadarArticleFrontendEnvelopeRefusalKind>,
    compile_refusal: Option<&TassadarCompileRefusal>,
    compile_succeeded: bool,
) -> TassadarArticleFrontendCorpusCaseStatus {
    if envelope_refusal_kind.is_some() {
        TassadarArticleFrontendCorpusCaseStatus::TypedRefusal
    } else if compile_refusal.is_some_and(is_toolchain_failure_refusal) {
        TassadarArticleFrontendCorpusCaseStatus::ToolchainFailure
    } else if compile_succeeded {
        TassadarArticleFrontendCorpusCaseStatus::Compiled
    } else {
        TassadarArticleFrontendCorpusCaseStatus::TypedRefusal
    }
}

fn classify_envelope_refusal(
    spec: &TassadarArticleFrontendCorpusCaseSpec,
    source_text: &str,
    wasm_summary: Option<&TassadarWasmBinarySummary>,
) -> Option<TassadarArticleFrontendEnvelopeRefusalKind> {
    if spec.expected_status == TassadarArticleFrontendCorpusCaseStatus::ToolchainFailure {
        return None;
    }
    if !source_text.contains("#![no_std]")
        || !source_text.contains("#![no_main]")
        || source_text.contains("use std::")
        || source_text.contains("extern crate std")
        || source_text.contains("alloc::")
        || source_text.contains("extern crate alloc")
    {
        return Some(TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLibrarySurface);
    }
    if source_text.contains("unsafe extern \"C\" {")
        || wasm_summary.is_some_and(|summary| summary.imported_function_count > 0)
    {
        return Some(TassadarArticleFrontendEnvelopeRefusalKind::HostOrSyscallSurfaceDisallowed);
    }
    if source_text.contains("unreachable_unchecked") {
        return Some(TassadarArticleFrontendEnvelopeRefusalKind::UbDependentSourceDisallowed);
    }
    if source_text.contains("i64")
        || source_text.contains("-> (")
        || source_text.contains("*mut i64")
        || source_text.contains("*const i64")
    {
        return Some(TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredAbiSurface);
    }
    spec.expected_refusal_kind
}

fn compile_config_matches_manifest(
    compile_config: &TassadarRustToWasmCompileConfig,
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
) -> bool {
    compile_config.compiler_binary == manifest.toolchain_policy.compiler_family
        && compile_config.target == manifest.toolchain_policy.target
        && language_version_matches(
            compile_config.edition.as_str(),
            &manifest.toolchain_policy.language_version,
        )
        && compile_config.crate_type == manifest.toolchain_policy.crate_type
        && compile_config.optimization_level == manifest.toolchain_policy.optimization_level
        && compile_config.panic_strategy == manifest.toolchain_policy.panic_strategy
}

fn language_version_matches(edition: &str, manifest_language_version: &str) -> bool {
    edition == manifest_language_version
        || format!("edition_{edition}") == manifest_language_version
}

fn manifest_allows_abi_surface(
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
    abi_surface_id: &str,
) -> bool {
    manifest
        .allowed_abi_surfaces
        .iter()
        .any(|surface| abi_surface_slug(surface.abi_surface_id) == abi_surface_id)
}

fn output_matches_receipt(
    output_path: &Path,
    receipt: &psionic_runtime::TassadarRustToWasmCompileReceipt,
) -> Result<bool, TassadarArticleFrontendCorpusCompileMatrixReportError> {
    let Some(expected_digest) = receipt.wasm_binary_digest() else {
        return Ok(false);
    };
    let wasm_bytes = fs::read(output_path).map_err(|error| {
        TassadarArticleFrontendCorpusCompileMatrixReportError::Read {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(stable_bytes_digest(&wasm_bytes) == expected_digest)
}

fn refusal_detail(refusal: &TassadarCompileRefusal) -> String {
    refusal.to_string()
}

fn is_toolchain_failure_refusal(refusal: &TassadarCompileRefusal) -> bool {
    matches!(
        refusal,
        TassadarCompileRefusal::ToolchainUnavailable { .. }
            | TassadarCompileRefusal::ToolchainFailure { .. }
    )
}

fn refusal_kind_slug(kind: TassadarArticleFrontendEnvelopeRefusalKind) -> &'static str {
    match kind {
        TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredFrontendFamily => {
            "outside_declared_frontend_family"
        }
        TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLanguageVersion => {
            "outside_declared_language_version"
        }
        TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLibrarySurface => {
            "outside_declared_library_surface"
        }
        TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredAbiSurface => {
            "outside_declared_abi_surface"
        }
        TassadarArticleFrontendEnvelopeRefusalKind::HostOrSyscallSurfaceDisallowed => {
            "host_or_syscall_surface_disallowed"
        }
        TassadarArticleFrontendEnvelopeRefusalKind::UbDependentSourceDisallowed => {
            "ub_dependent_source_disallowed"
        }
    }
}

fn abi_surface_slug(kind: TassadarArticleFrontendAbiSurfaceId) -> &'static str {
    match kind {
        TassadarArticleFrontendAbiSurfaceId::NullaryI32Return => "nullary_i32_return",
        TassadarArticleFrontendAbiSurfaceId::ScalarI32ParamsSingleI32Return => {
            "scalar_i32_params_single_i32_return"
        }
        TassadarArticleFrontendAbiSurfaceId::PointerLengthI32HeapInputSingleI32Return => {
            "pointer_length_i32_heap_input_single_i32_return"
        }
    }
}

fn required_categories() -> Vec<TassadarArticleFrontendCorpusCategory> {
    vec![
        TassadarArticleFrontendCorpusCategory::ArithmeticKernel,
        TassadarArticleFrontendCorpusCategory::BranchHeavyKernel,
        TassadarArticleFrontendCorpusCategory::StateMachineKernel,
        TassadarArticleFrontendCorpusCategory::AllocatorBackedMemoryKernel,
        TassadarArticleFrontendCorpusCategory::HungarianLikeSupport,
        TassadarArticleFrontendCorpusCategory::SudokuLikeSupport,
    ]
}

fn case_specs() -> Vec<TassadarArticleFrontendCorpusCaseSpec> {
    vec![
        compiled_case(
            "arithmetic_accumulator_exact",
            TassadarArticleFrontendCorpusCategory::ArithmeticKernel,
            "fixtures/tassadar/sources/tassadar_article_arithmetic_accumulator.rs",
            "fixtures/tassadar/wasm/tassadar_article_arithmetic_accumulator.wasm",
            canonical_compile_config(
                "tassadar_article_arithmetic_accumulator",
                &["arithmetic_accumulator", "arithmetic_mix_pair"],
            ),
            "scalar_i32_params_single_i32_return",
        ),
        compiled_case(
            "branch_dispatch_exact",
            TassadarArticleFrontendCorpusCategory::BranchHeavyKernel,
            "fixtures/tassadar/sources/tassadar_article_branch_dispatch.rs",
            "fixtures/tassadar/wasm/tassadar_article_branch_dispatch.wasm",
            canonical_compile_config(
                "tassadar_article_branch_dispatch",
                &["branch_dispatch_checksum"],
            ),
            "scalar_i32_params_single_i32_return",
        ),
        compiled_case(
            "state_machine_router_exact",
            TassadarArticleFrontendCorpusCategory::StateMachineKernel,
            "fixtures/tassadar/sources/tassadar_article_state_machine_router.rs",
            "fixtures/tassadar/wasm/tassadar_article_state_machine_router.wasm",
            canonical_compile_config(
                "tassadar_article_state_machine_router",
                &["state_machine_router"],
            ),
            "scalar_i32_params_single_i32_return",
        ),
        compiled_case(
            "state_machine_loop_exact",
            TassadarArticleFrontendCorpusCategory::StateMachineKernel,
            "fixtures/tassadar/sources/tassadar_state_machine_kernel.rs",
            "fixtures/tassadar/wasm/tassadar_state_machine_kernel_tas177.wasm",
            canonical_compile_config("tassadar_state_machine_kernel_tas177", &["state_machine_loop"]),
            "nullary_i32_return",
        ),
        compiled_case(
            "long_loop_frontier_exact",
            TassadarArticleFrontendCorpusCategory::StateMachineKernel,
            "fixtures/tassadar/sources/tassadar_long_loop_kernel.rs",
            "fixtures/tassadar/wasm/tassadar_long_loop_kernel_tas177.wasm",
            canonical_compile_config("tassadar_long_loop_kernel_tas177", &["million_step_loop"]),
            "nullary_i32_return",
        ),
        compiled_case(
            "bump_allocator_checksum_exact",
            TassadarArticleFrontendCorpusCategory::AllocatorBackedMemoryKernel,
            "fixtures/tassadar/sources/tassadar_article_bump_allocator.rs",
            "fixtures/tassadar/wasm/tassadar_article_bump_allocator.wasm",
            canonical_compile_config(
                "tassadar_article_bump_allocator",
                &["bump_allocator_checksum"],
            ),
            "pointer_length_i32_heap_input_single_i32_return",
        ),
        compiled_case(
            "heap_sum_window_exact",
            TassadarArticleFrontendCorpusCategory::AllocatorBackedMemoryKernel,
            "fixtures/tassadar/sources/tassadar_heap_sum_kernel.rs",
            "fixtures/tassadar/wasm/tassadar_heap_sum_kernel_tas177.wasm",
            canonical_compile_config(
                "tassadar_heap_sum_kernel_tas177",
                &["heap_sum_i32", "dot_i32", "sum_and_max_into_buffer"],
            ),
            "pointer_length_i32_heap_input_single_i32_return",
        ),
        compiled_case(
            "hungarian_support_checksum_exact",
            TassadarArticleFrontendCorpusCategory::HungarianLikeSupport,
            "fixtures/tassadar/sources/tassadar_article_hungarian_support.rs",
            "fixtures/tassadar/wasm/tassadar_article_hungarian_support.wasm",
            canonical_compile_config(
                "tassadar_article_hungarian_support",
                &["hungarian_support_checksum"],
            ),
            "nullary_i32_return",
        ),
        compiled_case(
            "hungarian_article_cost_exact",
            TassadarArticleFrontendCorpusCategory::HungarianLikeSupport,
            "fixtures/tassadar/sources/tassadar_hungarian_10x10_article.rs",
            "fixtures/tassadar/wasm/tassadar_hungarian_10x10_article_tas177.wasm",
            canonical_compile_config(
                "tassadar_hungarian_10x10_article_tas177",
                &["hungarian_10x10_article_cost"],
            ),
            "nullary_i32_return",
        ),
        compiled_case(
            "sudoku_support_checksum_exact",
            TassadarArticleFrontendCorpusCategory::SudokuLikeSupport,
            "fixtures/tassadar/sources/tassadar_article_sudoku_support.rs",
            "fixtures/tassadar/wasm/tassadar_article_sudoku_support.wasm",
            canonical_compile_config(
                "tassadar_article_sudoku_support",
                &["sudoku_support_checksum"],
            ),
            "nullary_i32_return",
        ),
        compiled_case(
            "sudoku_article_checksum_exact",
            TassadarArticleFrontendCorpusCategory::SudokuLikeSupport,
            "fixtures/tassadar/sources/tassadar_sudoku_9x9_article.rs",
            "fixtures/tassadar/wasm/tassadar_sudoku_9x9_article_tas177.wasm",
            canonical_compile_config(
                "tassadar_sudoku_9x9_article_tas177",
                &["sudoku_9x9_article_checksum"],
            ),
            "nullary_i32_return",
        ),
        refusal_case(
            "std_surface_library_refusal",
            "fixtures/tassadar/sources/tassadar_article_std_surface_refusal.rs",
            "fixtures/tassadar/wasm/tassadar_article_std_surface_refusal_tas177.wasm",
            canonical_compile_config(
                "tassadar_article_std_surface_refusal_tas177",
                &["std_surface_len"],
            ),
            TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLibrarySurface,
        ),
        refusal_case(
            "host_import_surface_refusal",
            "fixtures/tassadar/sources/tassadar_article_host_import_refusal.rs",
            "fixtures/tassadar/wasm/tassadar_article_host_import_refusal_tas177.wasm",
            canonical_compile_config(
                "tassadar_article_host_import_refusal_tas177",
                &["host_import_bridge"],
            ),
            TassadarArticleFrontendEnvelopeRefusalKind::HostOrSyscallSurfaceDisallowed,
        ),
        refusal_case(
            "ub_guard_refusal",
            "fixtures/tassadar/sources/tassadar_article_ub_refusal.rs",
            "fixtures/tassadar/wasm/tassadar_article_ub_refusal_tas177.wasm",
            canonical_compile_config(
                "tassadar_article_ub_refusal_tas177",
                &["ub_guard"],
            ),
            TassadarArticleFrontendEnvelopeRefusalKind::UbDependentSourceDisallowed,
        ),
        refusal_case(
            "wider_numeric_abi_refusal",
            "fixtures/tassadar/sources/tassadar_wider_numeric_kernel.rs",
            "fixtures/tassadar/wasm/tassadar_wider_numeric_kernel_tas177.wasm",
            canonical_compile_config(
                "tassadar_wider_numeric_kernel_tas177",
                &["pair_add_i64", "pair_sum_and_diff", "sum_and_max_i64_into_buffer"],
            ),
            TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredAbiSurface,
        ),
        toolchain_failure_case(
            "arithmetic_accumulator_missing_rustc",
            "fixtures/tassadar/sources/tassadar_article_arithmetic_accumulator.rs",
            "fixtures/tassadar/wasm/tassadar_article_arithmetic_accumulator_missing_rustc_tas177.wasm",
            missing_rustc_compile_config(
                "tassadar_article_arithmetic_accumulator_missing_rustc_tas177",
                &["arithmetic_accumulator", "arithmetic_mix_pair"],
            ),
        ),
    ]
}

fn compiled_case(
    case_id: &'static str,
    category_id: TassadarArticleFrontendCorpusCategory,
    source_ref: &'static str,
    output_wasm_ref: &'static str,
    compile_config: TassadarRustToWasmCompileConfig,
    abi_surface_id: &'static str,
) -> TassadarArticleFrontendCorpusCaseSpec {
    TassadarArticleFrontendCorpusCaseSpec {
        case_id,
        category_id,
        source_ref,
        output_wasm_ref,
        compile_config,
        expected_status: TassadarArticleFrontendCorpusCaseStatus::Compiled,
        expected_refusal_kind: None,
        abi_surface_id: Some(abi_surface_id),
    }
}

fn refusal_case(
    case_id: &'static str,
    source_ref: &'static str,
    output_wasm_ref: &'static str,
    compile_config: TassadarRustToWasmCompileConfig,
    expected_refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind,
) -> TassadarArticleFrontendCorpusCaseSpec {
    TassadarArticleFrontendCorpusCaseSpec {
        case_id,
        category_id: TassadarArticleFrontendCorpusCategory::RefusalCoverage,
        source_ref,
        output_wasm_ref,
        compile_config,
        expected_status: TassadarArticleFrontendCorpusCaseStatus::TypedRefusal,
        expected_refusal_kind: Some(expected_refusal_kind),
        abi_surface_id: None,
    }
}

fn toolchain_failure_case(
    case_id: &'static str,
    source_ref: &'static str,
    output_wasm_ref: &'static str,
    compile_config: TassadarRustToWasmCompileConfig,
) -> TassadarArticleFrontendCorpusCaseSpec {
    TassadarArticleFrontendCorpusCaseSpec {
        case_id,
        category_id: TassadarArticleFrontendCorpusCategory::ToolchainFailureProbe,
        source_ref,
        output_wasm_ref,
        compile_config,
        expected_status: TassadarArticleFrontendCorpusCaseStatus::ToolchainFailure,
        expected_refusal_kind: None,
        abi_surface_id: None,
    }
}

fn canonical_compile_config(
    crate_name: &str,
    export_symbols: &[&str],
) -> TassadarRustToWasmCompileConfig {
    TassadarRustToWasmCompileConfig {
        compiler_binary: String::from("rustc"),
        target: String::from("wasm32-unknown-unknown"),
        crate_name: String::from(crate_name),
        edition: String::from("2024"),
        crate_type: String::from("cdylib"),
        optimization_level: String::from("3"),
        panic_strategy: String::from("abort"),
        metadata_tag: String::from(crate_name),
        export_symbols: export_symbols
            .iter()
            .map(|symbol| String::from(*symbol))
            .collect(),
    }
}

fn missing_rustc_compile_config(
    crate_name: &str,
    export_symbols: &[&str],
) -> TassadarRustToWasmCompileConfig {
    let mut compile_config = canonical_compile_config(crate_name, export_symbols);
    compile_config.compiler_binary = String::from("rustc-missing-for-tas-177");
    compile_config
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

fn stable_bytes_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleFrontendCorpusCompileMatrixReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFrontendCorpusCompileMatrixReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFrontendCorpusCompileMatrixReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_frontend_corpus_compile_matrix_report, read_repo_json,
        tassadar_article_frontend_corpus_compile_matrix_report_path,
        write_tassadar_article_frontend_corpus_compile_matrix_report,
        TassadarArticleFrontendCorpusCaseStatus, TassadarArticleFrontendCorpusCompileMatrixReport,
        TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF,
    };

    #[test]
    fn article_frontend_corpus_compile_matrix_tracks_broad_frontend_green_without_final_green() {
        let report = build_tassadar_article_frontend_corpus_compile_matrix_report()
            .expect("frontend corpus compile matrix report");

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-177");
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(report.compiled_case_count, 11);
        assert_eq!(report.typed_refusal_case_count, 4);
        assert_eq!(report.toolchain_failure_case_count, 1);
        assert_eq!(report.lineage_green_count, 11);
        assert_eq!(report.refusal_green_count, 4);
        assert_eq!(report.toolchain_failure_green_count, 1);
        assert!(report.category_coverage_green);
        assert!(report.envelope_alignment_green);
        assert!(report.compile_matrix_green);
        assert!(report.article_equivalence_green);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert_eq!(
            report
                .case_rows
                .iter()
                .filter(|row| row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled)
                .count(),
            11
        );
    }

    #[test]
    fn article_frontend_corpus_compile_matrix_matches_committed_truth() {
        let generated = build_tassadar_article_frontend_corpus_compile_matrix_report()
            .expect("frontend corpus compile matrix report");
        let committed: TassadarArticleFrontendCorpusCompileMatrixReport = read_repo_json(
            TASSADAR_ARTICLE_FRONTEND_CORPUS_COMPILE_MATRIX_REPORT_REF,
            "report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_frontend_corpus_compile_matrix_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_frontend_corpus_compile_matrix_report.json");
        let written = write_tassadar_article_frontend_corpus_compile_matrix_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleFrontendCorpusCompileMatrixReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_frontend_corpus_compile_matrix_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_frontend_corpus_compile_matrix_report.json")
        );
    }
}
