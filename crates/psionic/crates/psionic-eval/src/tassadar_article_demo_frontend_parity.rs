use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

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
    TassadarCompilerToolchainIdentity, TassadarRustToWasmCompileConfig,
    TassadarRustToWasmCompileOutcome, TassadarRustToWasmCompileReceipt, TassadarWasmBinarySummary,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarRustSourceCanonCaseStatus,
    TassadarRustSourceCanonReport, TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_RUST_SOURCE_CANON_REPORT_REF,
};

pub const TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_demo_frontend_parity_report.json";
pub const TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_CHECKER_REF: &str =
    "scripts/check-tassadar-article-demo-frontend-parity.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-178";
const HUNGARIAN_REPRODUCER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_hungarian_10x10_article_reproducer_report.json";
const SUDOKU_REPRODUCER_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_sudoku_9x9_article_reproducer_report.json";
static ARTICLE_DEMO_FRONTEND_PARITY_BUILD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleDemoFrontendFamily {
    Hungarian10x10,
    Sudoku9x9,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoFrontendParityAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoFrontendParityRow {
    pub demo_id: TassadarArticleDemoFrontendFamily,
    pub source_case_id: String,
    pub source_workload_family_id: String,
    pub canonical_workload_family_id: String,
    pub canonical_case_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub source_receipt_digest: String,
    pub compile_receipt_digest: String,
    pub compile_config_digest: String,
    pub compile_pipeline_features: Vec<String>,
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    pub toolchain_digest: String,
    pub abi_surface_id: String,
    pub wasm_binary_ref: String,
    pub wasm_binary_digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm_binary_summary: Option<TassadarWasmBinarySummary>,
    pub source_canon_compiled: bool,
    pub source_ref_green: bool,
    pub source_digest_parity_green: bool,
    pub source_compile_receipt_parity_green: bool,
    pub canonical_wasm_parity_green: bool,
    pub manifest_alignment_green: bool,
    pub zero_import_wasm_green: bool,
    pub output_digest_matches_receipt: bool,
    pub canonical_case_id_green: bool,
    pub canonical_workload_identity_green: bool,
    pub row_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoFrontendParityRefusalProbe {
    pub row_id: String,
    pub demo_id: TassadarArticleDemoFrontendFamily,
    pub source_ref: String,
    pub source_digest: String,
    pub output_wasm_ref: String,
    pub compile_config_digest: String,
    pub compile_pipeline_features: Vec<String>,
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    pub toolchain_digest: String,
    pub expected_refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_refusal_kind: Option<TassadarArticleFrontendEnvelopeRefusalKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_refusal_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_refusal_detail: Option<String>,
    pub refusal_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleDemoFrontendParityReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleDemoFrontendParityAcceptanceGateTie,
    pub manifest_ref: String,
    pub manifest: TassadarArticleFrontendCompilerEnvelopeManifest,
    pub source_canon_report_ref: String,
    pub demo_rows: Vec<TassadarArticleDemoFrontendParityRow>,
    pub refusal_probes: Vec<TassadarArticleDemoFrontendParityRefusalProbe>,
    pub compiled_demo_count: usize,
    pub green_demo_count: usize,
    pub refusal_probe_green_count: usize,
    pub source_compile_receipt_parity_green: bool,
    pub workload_identity_parity_green: bool,
    pub unsupported_variant_refusal_green: bool,
    pub demo_frontend_parity_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Clone)]
struct TassadarArticleDemoFrontendSpec {
    demo_id: TassadarArticleDemoFrontendFamily,
    source_case_id: &'static str,
    source_ref: &'static str,
    output_wasm_ref: &'static str,
    compile_config: TassadarRustToWasmCompileConfig,
    abi_surface_id: &'static str,
    expected_canonical_case_id: &'static str,
    expected_canonical_workload_family_id: &'static str,
    reproducer_report_ref: &'static str,
}

#[derive(Clone)]
struct TassadarArticleDemoFrontendRefusalSpec {
    row_id: &'static str,
    demo_id: TassadarArticleDemoFrontendFamily,
    source_ref: &'static str,
    output_wasm_ref: &'static str,
    compile_config: TassadarRustToWasmCompileConfig,
    expected_refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct TassadarArticleDemoFrontendReproducerProjection {
    workload_family_id: String,
    source_ref: String,
    source_digest: String,
    source_receipt_digest: String,
    wasm_binary_ref: String,
    wasm_binary_digest: String,
    canonical_case_id: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleDemoFrontendParityReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error("internal TAS-178 invariant failed: {detail}")]
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

pub fn build_tassadar_article_demo_frontend_parity_report(
) -> Result<TassadarArticleDemoFrontendParityReport, TassadarArticleDemoFrontendParityReportError> {
    let _guard = ARTICLE_DEMO_FRONTEND_PARITY_BUILD_LOCK
        .lock()
        .expect("build lock");
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let manifest = build_tassadar_article_frontend_compiler_envelope_manifest();
    let source_canon: TassadarRustSourceCanonReport = read_repo_json(
        TASSADAR_RUST_SOURCE_CANON_REPORT_REF,
        "tassadar_rust_source_canon_report",
    )?;
    let demo_rows = demo_specs()
        .into_iter()
        .map(|spec| build_demo_row(&manifest, &source_canon, &spec))
        .collect::<Result<Vec<_>, _>>()?;
    let refusal_probes = refusal_specs()
        .into_iter()
        .map(|spec| build_refusal_probe(&manifest, &spec))
        .collect::<Result<Vec<_>, _>>()?;
    let compiled_demo_count = demo_rows
        .iter()
        .filter(|row| !row.wasm_binary_ref.is_empty())
        .count();
    let green_demo_count = demo_rows.iter().filter(|row| row.row_green).count();
    let refusal_probe_green_count = refusal_probes
        .iter()
        .filter(|row| row.refusal_green)
        .count();
    let source_compile_receipt_parity_green = demo_rows.iter().all(|row| {
        row.source_canon_compiled
            && row.source_ref_green
            && row.source_digest_parity_green
            && row.source_compile_receipt_parity_green
            && row.canonical_wasm_parity_green
            && row.output_digest_matches_receipt
    });
    let workload_identity_parity_green = demo_rows.iter().all(|row| {
        row.canonical_case_id_green && row.canonical_workload_identity_green && row.row_green
    });
    let unsupported_variant_refusal_green = refusal_probes.iter().all(|probe| probe.refusal_green);
    let acceptance_gate_tie = build_acceptance_gate_tie(&acceptance_gate)?;
    let demo_frontend_parity_green = acceptance_gate_tie.tied_requirement_satisfied
        && compiled_demo_count == demo_rows.len()
        && green_demo_count == demo_rows.len()
        && refusal_probe_green_count == refusal_probes.len()
        && source_compile_receipt_parity_green
        && workload_identity_parity_green
        && unsupported_variant_refusal_green;

    let mut report = TassadarArticleDemoFrontendParityReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_demo_frontend_parity.report.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_CHECKER_REF),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        manifest_ref: String::from(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF),
        manifest,
        source_canon_report_ref: String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
        demo_rows,
        refusal_probes,
        compiled_demo_count,
        green_demo_count,
        refusal_probe_green_count,
        source_compile_receipt_parity_green,
        workload_identity_parity_green,
        unsupported_variant_refusal_green,
        demo_frontend_parity_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && demo_frontend_parity_green,
        claim_boundary: String::from(
            "this report closes TAS-178 only. It proves that the canonical Hungarian and Sudoku article demo sources compile through the declared public frontend/compiler envelope and stay bound to the same canonical compiled-executor case and workload identities already used by the bounded reproducers. It does not yet claim final Hungarian fast-route parity, Arto or benchmark-wide hard-Sudoku closure, arbitrary-program closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article demo frontend parity now records tied_requirement_satisfied={}, compiled_demos={}/{}, refusal_probes={}/{}, source_compile_receipt_parity_green={}, workload_identity_parity_green={}, unsupported_variant_refusal_green={}, demo_frontend_parity_green={}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.green_demo_count,
        report.compiled_demo_count,
        report.refusal_probe_green_count,
        report.refusal_probes.len(),
        report.source_compile_receipt_parity_green,
        report.workload_identity_parity_green,
        report.unsupported_variant_refusal_green,
        report.demo_frontend_parity_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_demo_frontend_parity_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_article_demo_frontend_parity_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF)
}

pub fn write_tassadar_article_demo_frontend_parity_report(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleDemoFrontendParityReport, TassadarArticleDemoFrontendParityReportError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleDemoFrontendParityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_demo_frontend_parity_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleDemoFrontendParityReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_acceptance_gate_tie(
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
) -> Result<
    TassadarArticleDemoFrontendParityAcceptanceGateTie,
    TassadarArticleDemoFrontendParityReportError,
> {
    let tied_requirement_satisfied = acceptance_gate
        .green_requirement_ids
        .iter()
        .any(|requirement_id| requirement_id == TIED_REQUIREMENT_ID);
    if !tied_requirement_satisfied {
        return Err(TassadarArticleDemoFrontendParityReportError::Invariant {
            detail: format!(
                "acceptance gate does not yet treat `{}` as green",
                TIED_REQUIREMENT_ID
            ),
        });
    }
    Ok(TassadarArticleDemoFrontendParityAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied,
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    })
}

fn build_demo_row(
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
    source_canon: &TassadarRustSourceCanonReport,
    spec: &TassadarArticleDemoFrontendSpec,
) -> Result<TassadarArticleDemoFrontendParityRow, TassadarArticleDemoFrontendParityReportError> {
    let source_case = source_canon
        .cases
        .iter()
        .find(|case| case.case_id == spec.source_case_id)
        .ok_or_else(|| TassadarArticleDemoFrontendParityReportError::Invariant {
            detail: format!("missing source canon case `{}`", spec.source_case_id),
        })?;
    let reproducer: TassadarArticleDemoFrontendReproducerProjection =
        read_repo_json(spec.reproducer_report_ref, "article_demo_reproducer_report")?;
    let source_path = repo_root().join(spec.source_ref);
    let source_bytes = fs::read(&source_path).map_err(|error| {
        TassadarArticleDemoFrontendParityReportError::Read {
            path: source_path.display().to_string(),
            error,
        }
    })?;
    let output_path = canonical_demo_output_path(spec);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleDemoFrontendParityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let compile_receipt = compile_tassadar_rust_source_to_wasm_receipt(
        spec.source_ref,
        &source_bytes,
        &output_path,
        &spec.compile_config,
    );
    let toolchain_identity = compile_receipt.toolchain_identity.clone();
    let toolchain_digest = stable_digest(
        b"psionic_tassadar_article_demo_frontend_parity_toolchain|",
        &toolchain_identity,
    );
    let normalized_compile_receipt_digest =
        normalized_compile_receipt_digest(&compile_receipt, spec.output_wasm_ref);
    let source_canon_compiled = source_case.status == TassadarRustSourceCanonCaseStatus::Compiled;
    let source_ref_green = compile_receipt.source_identity.source_name == spec.source_ref
        && source_case.source_ref == spec.source_ref
        && reproducer.source_ref == spec.source_ref;
    let source_digest_parity_green = compile_receipt.source_identity.source_digest
        == source_case.source_digest
        && source_case.source_digest == reproducer.source_digest;
    let source_compile_receipt_parity_green = compile_receipt.succeeded();
    let canonical_wasm_parity_green = source_case.wasm_binary_ref.as_deref()
        == Some(reproducer.wasm_binary_ref.as_str())
        && source_case.wasm_binary_digest.as_deref()
            == Some(reproducer.wasm_binary_digest.as_str())
        && !reproducer.wasm_binary_ref.is_empty()
        && !reproducer.wasm_binary_digest.is_empty();
    let manifest_alignment_green = compile_receipt.succeeded()
        && compile_config_matches_manifest(&spec.compile_config, manifest)
        && manifest_allows_abi_surface(manifest, spec.abi_surface_id);
    let zero_import_wasm_green = compile_receipt
        .wasm_binary_summary()
        .is_some_and(|summary| summary.imported_function_count == 0);
    let output_digest_matches_receipt = output_matches_receipt(&output_path, &compile_receipt)?;
    let canonical_case_id_green = reproducer.canonical_case_id == spec.expected_canonical_case_id;
    let canonical_workload_identity_green =
        reproducer.workload_family_id == spec.expected_canonical_workload_family_id;
    let row_green = source_canon_compiled
        && source_ref_green
        && source_digest_parity_green
        && source_compile_receipt_parity_green
        && canonical_wasm_parity_green
        && manifest_alignment_green
        && zero_import_wasm_green
        && output_digest_matches_receipt
        && canonical_case_id_green
        && canonical_workload_identity_green;

    Ok(TassadarArticleDemoFrontendParityRow {
        demo_id: spec.demo_id,
        source_case_id: String::from(spec.source_case_id),
        source_workload_family_id: source_case.workload_family_id.clone(),
        canonical_workload_family_id: reproducer.workload_family_id,
        canonical_case_id: reproducer.canonical_case_id,
        source_ref: String::from(spec.source_ref),
        source_digest: stable_bytes_digest(&source_bytes),
        source_receipt_digest: source_case.receipt_digest.clone(),
        compile_receipt_digest: normalized_compile_receipt_digest,
        compile_config_digest: spec.compile_config.stable_digest(),
        compile_pipeline_features: spec.compile_config.pipeline_features(),
        toolchain_identity,
        toolchain_digest,
        abi_surface_id: String::from(spec.abi_surface_id),
        wasm_binary_ref: String::from(spec.output_wasm_ref),
        wasm_binary_digest: compile_receipt
            .wasm_binary_digest()
            .unwrap_or_default()
            .to_string(),
        wasm_binary_summary: compile_receipt.wasm_binary_summary().cloned(),
        source_canon_compiled,
        source_ref_green,
        source_digest_parity_green,
        source_compile_receipt_parity_green,
        canonical_wasm_parity_green,
        manifest_alignment_green,
        zero_import_wasm_green,
        output_digest_matches_receipt,
        canonical_case_id_green,
        canonical_workload_identity_green,
        row_green,
        detail: format!(
            "compiled `{}` through the declared article envelope into `{}` beside the existing bounded route and kept source identity plus canonical case-id and workload-id binding with the bounded reproducer",
            spec.source_case_id, spec.output_wasm_ref
        ),
    })
}

fn build_refusal_probe(
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
    spec: &TassadarArticleDemoFrontendRefusalSpec,
) -> Result<
    TassadarArticleDemoFrontendParityRefusalProbe,
    TassadarArticleDemoFrontendParityReportError,
> {
    let source_path = repo_root().join(spec.source_ref);
    let source_bytes = fs::read(&source_path).map_err(|error| {
        TassadarArticleDemoFrontendParityReportError::Read {
            path: source_path.display().to_string(),
            error,
        }
    })?;
    let source_text = String::from_utf8_lossy(&source_bytes);
    let output_path = repo_root().join(spec.output_wasm_ref);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleDemoFrontendParityReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let compile_receipt = compile_tassadar_rust_source_to_wasm_receipt(
        spec.source_ref,
        &source_bytes,
        &output_path,
        &spec.compile_config,
    );
    let toolchain_identity = compile_receipt.toolchain_identity.clone();
    let toolchain_digest = stable_digest(
        b"psionic_tassadar_article_demo_frontend_parity_refusal_toolchain|",
        &toolchain_identity,
    );
    let actual_refusal_kind =
        classify_envelope_refusal(source_text.as_ref(), compile_receipt.wasm_binary_summary());
    let compile_refusal_kind = compile_receipt
        .refusal()
        .map(|refusal| refusal.kind_slug().to_string());
    let compile_refusal_detail = compile_receipt.refusal().map(refusal_detail);
    let refusal_green = compile_config_matches_manifest(&spec.compile_config, manifest)
        && actual_refusal_kind == Some(spec.expected_refusal_kind);

    Ok(TassadarArticleDemoFrontendParityRefusalProbe {
        row_id: String::from(spec.row_id),
        demo_id: spec.demo_id,
        source_ref: String::from(spec.source_ref),
        source_digest: stable_bytes_digest(&source_bytes),
        output_wasm_ref: String::from(spec.output_wasm_ref),
        compile_config_digest: spec.compile_config.stable_digest(),
        compile_pipeline_features: spec.compile_config.pipeline_features(),
        toolchain_identity,
        toolchain_digest,
        expected_refusal_kind: spec.expected_refusal_kind,
        actual_refusal_kind,
        compile_refusal_kind,
        compile_refusal_detail,
        refusal_green,
        detail: format!(
            "unsupported {:?} demo variant keeps refusal_kind={} explicit under the declared article envelope",
            spec.demo_id,
            refusal_kind_slug(spec.expected_refusal_kind)
        ),
    })
}

fn classify_envelope_refusal(
    source_text: &str,
    wasm_summary: Option<&TassadarWasmBinarySummary>,
) -> Option<TassadarArticleFrontendEnvelopeRefusalKind> {
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
    None
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
) -> Result<bool, TassadarArticleDemoFrontendParityReportError> {
    let Some(expected_digest) = receipt.wasm_binary_digest() else {
        return Ok(false);
    };
    let wasm_bytes = fs::read(output_path).map_err(|error| {
        TassadarArticleDemoFrontendParityReportError::Read {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(stable_bytes_digest(&wasm_bytes) == expected_digest)
}

fn normalized_compile_receipt_digest(
    receipt: &TassadarRustToWasmCompileReceipt,
    canonical_wasm_ref: &str,
) -> String {
    let mut normalized = receipt.clone();
    if let TassadarRustToWasmCompileOutcome::Succeeded {
        wasm_binary_ref, ..
    } = &mut normalized.outcome
    {
        *wasm_binary_ref = String::from(canonical_wasm_ref);
    }
    normalized.receipt_digest =
        stable_digest(b"tassadar_rust_to_wasm_compile_receipt|", &normalized);
    normalized.receipt_digest
}

fn refusal_detail(refusal: &TassadarCompileRefusal) -> String {
    refusal.to_string()
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

fn demo_specs() -> Vec<TassadarArticleDemoFrontendSpec> {
    vec![
        TassadarArticleDemoFrontendSpec {
            demo_id: TassadarArticleDemoFrontendFamily::Hungarian10x10,
            source_case_id: "hungarian_10x10_article",
            source_ref: "fixtures/tassadar/sources/tassadar_hungarian_10x10_article.rs",
            output_wasm_ref: "fixtures/tassadar/wasm/tassadar_hungarian_10x10_article_tas178.wasm",
            compile_config: TassadarRustToWasmCompileConfig::canonical_hungarian_10x10_article(),
            abi_surface_id: "nullary_i32_return",
            expected_canonical_case_id: "hungarian_10x10_test_a",
            expected_canonical_workload_family_id:
                "tassadar.wasm.hungarian_10x10_matching.v1.compiled_executor",
            reproducer_report_ref: HUNGARIAN_REPRODUCER_REPORT_REF,
        },
        TassadarArticleDemoFrontendSpec {
            demo_id: TassadarArticleDemoFrontendFamily::Sudoku9x9,
            source_case_id: "sudoku_9x9_article",
            source_ref: "fixtures/tassadar/sources/tassadar_sudoku_9x9_article.rs",
            output_wasm_ref: "fixtures/tassadar/wasm/tassadar_sudoku_9x9_article_tas178.wasm",
            compile_config: TassadarRustToWasmCompileConfig::canonical_sudoku_9x9_article(),
            abi_surface_id: "nullary_i32_return",
            expected_canonical_case_id: "sudoku_9x9_test_a",
            expected_canonical_workload_family_id:
                "tassadar.wasm.sudoku_9x9_search.v1.compiled_executor",
            reproducer_report_ref: SUDOKU_REPRODUCER_REPORT_REF,
        },
    ]
}

fn refusal_specs() -> Vec<TassadarArticleDemoFrontendRefusalSpec> {
    vec![
        TassadarArticleDemoFrontendRefusalSpec {
            row_id: "hungarian_10x10_article_std_variant_refusal",
            demo_id: TassadarArticleDemoFrontendFamily::Hungarian10x10,
            source_ref: "fixtures/tassadar/sources/tassadar_hungarian_10x10_article_std_refusal.rs",
            output_wasm_ref:
                "fixtures/tassadar/wasm/tassadar_hungarian_10x10_article_std_refusal_tas178.wasm",
            compile_config: canonical_compile_config(
                "tassadar_hungarian_10x10_article_std_refusal_tas178",
                &["hungarian_10x10_article_std_refusal_cost"],
            ),
            expected_refusal_kind:
                TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLibrarySurface,
        },
        TassadarArticleDemoFrontendRefusalSpec {
            row_id: "sudoku_9x9_article_host_import_variant_refusal",
            demo_id: TassadarArticleDemoFrontendFamily::Sudoku9x9,
            source_ref:
                "fixtures/tassadar/sources/tassadar_sudoku_9x9_article_host_import_refusal.rs",
            output_wasm_ref:
                "fixtures/tassadar/wasm/tassadar_sudoku_9x9_article_host_import_refusal_tas178.wasm",
            compile_config: canonical_compile_config(
                "tassadar_sudoku_9x9_article_host_import_refusal_tas178",
                &["sudoku_9x9_article_host_import_checksum"],
            ),
            expected_refusal_kind:
                TassadarArticleFrontendEnvelopeRefusalKind::HostOrSyscallSurfaceDisallowed,
        },
    ]
}

fn canonical_demo_output_path(spec: &TassadarArticleDemoFrontendSpec) -> PathBuf {
    repo_root().join(spec.output_wasm_ref)
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

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleDemoFrontendParityReportError> {
    let path = repo_root().join(relative_path);
    let bytes =
        fs::read(&path).map_err(|error| TassadarArticleDemoFrontendParityReportError::Read {
            path: path.display().to_string(),
            error,
        })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleDemoFrontendParityReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_demo_frontend_parity_report, read_repo_json,
        tassadar_article_demo_frontend_parity_report_path,
        write_tassadar_article_demo_frontend_parity_report,
        TassadarArticleDemoFrontendParityReport, TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF,
    };

    #[test]
    fn article_demo_frontend_parity_closes_demo_source_layer_without_final_green() {
        let report = build_tassadar_article_demo_frontend_parity_report().expect("report");

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-178");
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert_eq!(report.compiled_demo_count, 2);
        assert_eq!(report.green_demo_count, 2);
        assert_eq!(report.refusal_probe_green_count, 2);
        assert!(report.source_compile_receipt_parity_green);
        assert!(report.workload_identity_parity_green);
        assert!(report.unsupported_variant_refusal_green);
        assert!(report.demo_frontend_parity_green);
        assert!(report.article_equivalence_green);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
    }

    #[test]
    fn article_demo_frontend_parity_matches_committed_truth() {
        let generated = build_tassadar_article_demo_frontend_parity_report().expect("report");
        let committed: TassadarArticleDemoFrontendParityReport = read_repo_json(
            TASSADAR_ARTICLE_DEMO_FRONTEND_PARITY_REPORT_REF,
            "article_demo_frontend_parity_report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_demo_frontend_parity_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_demo_frontend_parity_report.json");
        let written =
            write_tassadar_article_demo_frontend_parity_report(&output_path).expect("write report");
        let persisted: TassadarArticleDemoFrontendParityReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_demo_frontend_parity_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_demo_frontend_parity_report.json")
        );
    }
}
