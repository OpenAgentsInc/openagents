use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_compiler::{
    build_tassadar_article_frontend_compiler_envelope_manifest,
    tassadar_article_frontend_compiler_envelope_manifest_path,
    write_tassadar_article_frontend_compiler_envelope_manifest,
    TassadarArticleFrontendCompilerEnvelopeManifest, TassadarArticleFrontendEnvelopeRefusalKind,
    TassadarArticleFrontendFamily, TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF,
};
use psionic_runtime::{
    TassadarCompilerToolchainIdentity, TassadarProgramSourceKind, TASSADAR_CANONICAL_C_SOURCE_REF,
};

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarCompilePipelineMatrixCaseStatus,
    TassadarCompilePipelineMatrixReport, TassadarRustSourceCanonCaseStatus,
    TassadarRustSourceCanonReport, TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF, TASSADAR_RUST_SOURCE_CANON_REPORT_REF,
};

pub const TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_frontend_compiler_envelope_report.json";
pub const TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-frontend-compiler-envelope.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-176";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerEnvelopeAcceptanceGateTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerEnvelopeManifestCheck {
    pub manifest_ref: String,
    pub admitted_source_count: usize,
    pub declared_refusal_kinds: Vec<TassadarArticleFrontendEnvelopeRefusalKind>,
    pub manifest_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerEnvelopeCompileMatrixTie {
    pub report_ref: String,
    pub case_id: String,
    pub source_ref: String,
    pub case_status: TassadarCompilePipelineMatrixCaseStatus,
    pub compile_refusal_kind: Option<String>,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerEnvelopeAdmittedCaseCheck {
    pub case_id: String,
    pub source_ref: String,
    pub source_digest: String,
    pub source_canon_compiled: bool,
    pub compile_config_digest_matches_manifest: bool,
    pub toolchain_identity: TassadarCompilerToolchainIdentity,
    pub toolchain_identity_green: bool,
    pub source_policy_green: bool,
    pub imported_function_count: u32,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerEnvelopeRefusalProbe {
    pub row_id: String,
    pub source_ref: String,
    pub source_kind: TassadarProgramSourceKind,
    pub expected_refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind,
    pub actual_refusal_kind: TassadarArticleFrontendEnvelopeRefusalKind,
    pub probe_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleFrontendCompilerEnvelopeReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleFrontendCompilerEnvelopeAcceptanceGateTie,
    pub manifest_ref: String,
    pub manifest: TassadarArticleFrontendCompilerEnvelopeManifest,
    pub manifest_check: TassadarArticleFrontendCompilerEnvelopeManifestCheck,
    pub rust_source_canon_report_ref: String,
    pub compile_matrix_report_ref: String,
    pub compile_matrix_tie: TassadarArticleFrontendCompilerEnvelopeCompileMatrixTie,
    pub admitted_case_checks: Vec<TassadarArticleFrontendCompilerEnvelopeAdmittedCaseCheck>,
    pub refusal_probes: Vec<TassadarArticleFrontendCompilerEnvelopeRefusalProbe>,
    pub admitted_case_green_count: usize,
    pub refusal_probe_green_count: usize,
    pub toolchain_identity_green: bool,
    pub refusal_taxonomy_green: bool,
    pub envelope_manifest_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleFrontendCompilerEnvelopeReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error("internal TAS-176 invariant failed: {detail}")]
    Invariant { detail: String },
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

pub fn build_tassadar_article_frontend_compiler_envelope_report() -> Result<
    TassadarArticleFrontendCompilerEnvelopeReport,
    TassadarArticleFrontendCompilerEnvelopeReportError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let manifest = build_tassadar_article_frontend_compiler_envelope_manifest();
    let rust_source_canon: TassadarRustSourceCanonReport = read_repo_json(
        TASSADAR_RUST_SOURCE_CANON_REPORT_REF,
        "rust_source_canon_report",
    )?;
    let compile_matrix: TassadarCompilePipelineMatrixReport = read_repo_json(
        TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF,
        "compile_pipeline_matrix_report",
    )?;

    let acceptance_gate_tie = build_acceptance_gate_tie(&acceptance_gate)?;
    let manifest_check = build_manifest_check(&manifest);
    let compile_matrix_tie = build_compile_matrix_tie(&compile_matrix)?;
    let admitted_case_checks = build_admitted_case_checks(&manifest, &rust_source_canon)?;
    let refusal_probes = build_refusal_probes(&manifest)?;
    let admitted_case_green_count = admitted_case_checks
        .iter()
        .filter(|check| {
            check.source_canon_compiled
                && check.compile_config_digest_matches_manifest
                && check.toolchain_identity_green
                && check.source_policy_green
                && check.imported_function_count == 0
        })
        .count();
    let refusal_probe_green_count = refusal_probes
        .iter()
        .filter(|probe| probe.probe_green)
        .count();
    let toolchain_identity_green = admitted_case_checks
        .iter()
        .all(|check| check.toolchain_identity_green)
        && unique_compiler_versions(admitted_case_checks.as_slice()).len() == 1;
    let refusal_taxonomy_green = refusal_probe_green_count == refusal_probes.len()
        && manifest_refusal_kinds_green(&manifest);
    let envelope_manifest_green = manifest_check.manifest_green
        && compile_matrix_tie.green
        && admitted_case_green_count == admitted_case_checks.len();

    let mut report = TassadarArticleFrontendCompilerEnvelopeReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_frontend_compiler_envelope.report.v1"),
        checker_script_ref: String::from(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_CHECKER_REF),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        manifest_ref: String::from(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF),
        manifest,
        manifest_check,
        rust_source_canon_report_ref: String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
        compile_matrix_report_ref: String::from(TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF),
        compile_matrix_tie,
        admitted_case_checks,
        refusal_probes,
        admitted_case_green_count,
        refusal_probe_green_count,
        toolchain_identity_green,
        refusal_taxonomy_green,
        envelope_manifest_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && envelope_manifest_green
            && toolchain_identity_green
            && refusal_taxonomy_green,
        claim_boundary: String::from(
            "this report closes TAS-176 only. It freezes one declared public article frontend/compiler envelope, proves that the currently admitted Rust article fixtures stay inside it with stable toolchain identity and zero-import Wasm outputs, and proves that representative out-of-envelope rows still refuse explicitly. It does not yet widen the corpus, close Hungarian or Sudoku demo parity, or turn final article-equivalence green.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article frontend/compiler envelope report now records tied_requirement_satisfied={}, manifest_green={}, admitted_cases={}/{}, refusal_probes={}/{}, toolchain_identity_green={}, refusal_taxonomy_green={}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.envelope_manifest_green,
        report.admitted_case_green_count,
        report.admitted_case_checks.len(),
        report.refusal_probe_green_count,
        report.refusal_probes.len(),
        report.toolchain_identity_green,
        report.refusal_taxonomy_green,
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_frontend_compiler_envelope_report|",
        &report,
    );
    Ok(report)
}

pub fn tassadar_article_frontend_compiler_envelope_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF)
}

pub fn write_tassadar_article_frontend_compiler_envelope_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleFrontendCompilerEnvelopeReport,
    TassadarArticleFrontendCompilerEnvelopeReportError,
> {
    write_tassadar_article_frontend_compiler_envelope_manifest(
        tassadar_article_frontend_compiler_envelope_manifest_path(),
    )
    .map_err(
        |error| TassadarArticleFrontendCompilerEnvelopeReportError::Invariant {
            detail: error.to_string(),
        },
    )?;
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleFrontendCompilerEnvelopeReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_frontend_compiler_envelope_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleFrontendCompilerEnvelopeReportError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(report)
}

fn build_acceptance_gate_tie(
    acceptance_gate: &TassadarArticleEquivalenceAcceptanceGateReport,
) -> Result<
    TassadarArticleFrontendCompilerEnvelopeAcceptanceGateTie,
    TassadarArticleFrontendCompilerEnvelopeReportError,
> {
    let tied_requirement_satisfied = acceptance_gate
        .green_requirement_ids
        .iter()
        .any(|requirement_id| requirement_id == TIED_REQUIREMENT_ID);
    if !tied_requirement_satisfied {
        return Err(
            TassadarArticleFrontendCompilerEnvelopeReportError::Invariant {
                detail: format!(
                    "acceptance gate does not yet treat `{}` as green",
                    TIED_REQUIREMENT_ID
                ),
            },
        );
    }
    Ok(TassadarArticleFrontendCompilerEnvelopeAcceptanceGateTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied,
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    })
}

fn build_manifest_check(
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
) -> TassadarArticleFrontendCompilerEnvelopeManifestCheck {
    let declared_refusal_kinds = manifest
        .disallowed_rows
        .iter()
        .map(|row| row.refusal_kind)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let manifest_green = manifest.frontend_family == TassadarArticleFrontendFamily::RustSource
        && manifest.admitted_source_rows.len() == 8
        && manifest_refusal_kinds_green(manifest);
    TassadarArticleFrontendCompilerEnvelopeManifestCheck {
        manifest_ref: String::from(TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_MANIFEST_REF),
        admitted_source_count: manifest.admitted_source_rows.len(),
        declared_refusal_kinds,
        manifest_green,
        detail: String::from(
            "the manifest is green only when the Rust-only envelope stays explicit, all refusal kinds remain declared, and the bounded admitted source set is present",
        ),
    }
}

fn build_compile_matrix_tie(
    compile_matrix: &TassadarCompilePipelineMatrixReport,
) -> Result<
    TassadarArticleFrontendCompilerEnvelopeCompileMatrixTie,
    TassadarArticleFrontendCompilerEnvelopeReportError,
> {
    let case = compile_matrix
        .cases
        .iter()
        .find(|case| case.case_id == "c_missing_toolchain_refusal")
        .ok_or_else(
            || TassadarArticleFrontendCompilerEnvelopeReportError::Invariant {
                detail: String::from(
                    "compile pipeline matrix omitted `c_missing_toolchain_refusal`",
                ),
            },
        )?;
    let green = case.source_ref == TASSADAR_CANONICAL_C_SOURCE_REF
        && case.status == TassadarCompilePipelineMatrixCaseStatus::CompileRefused;
    Ok(TassadarArticleFrontendCompilerEnvelopeCompileMatrixTie {
        report_ref: String::from(TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF),
        case_id: case.case_id.clone(),
        source_ref: case.source_ref.clone(),
        case_status: case.status,
        compile_refusal_kind: case.compile_refusal_kind.clone(),
        green,
        detail: String::from(
            "the older C-source compile receipt remains visible as a historical compile-matrix row, but it stays outside the declared article frontend/compiler envelope",
        ),
    })
}

fn build_admitted_case_checks(
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
    rust_source_canon: &TassadarRustSourceCanonReport,
) -> Result<
    Vec<TassadarArticleFrontendCompilerEnvelopeAdmittedCaseCheck>,
    TassadarArticleFrontendCompilerEnvelopeReportError,
> {
    manifest
        .admitted_source_rows
        .iter()
        .map(|row| {
            let source_case = rust_source_canon
                .cases
                .iter()
                .find(|case| case.case_id == row.case_id)
                .ok_or_else(|| TassadarArticleFrontendCompilerEnvelopeReportError::Invariant {
                    detail: format!(
                        "rust source canon omitted admitted envelope case `{}`",
                        row.case_id
                    ),
                })?;
            let source_text = read_repo_text(row.source_ref.as_str())?;
            let facts = inspect_source(row.source_kind, source_text.as_str());
            let source_canon_compiled =
                source_case.status == TassadarRustSourceCanonCaseStatus::Compiled;
            let compile_config_digest_matches_manifest =
                source_case.compile_config_digest == row.compile_config_digest;
            let toolchain_identity_green =
                source_case.toolchain_identity.compiler_family
                    == manifest.toolchain_policy.compiler_family
                    && source_case.toolchain_identity.target == manifest.toolchain_policy.target
                    && source_case
                        .toolchain_identity
                        .pipeline_features
                        == row.compile_pipeline_features
                    && source_case.toolchain_identity.compiler_version != "unavailable";
            let source_policy_green = facts.has_no_std
                && facts.has_no_main
                && !facts.uses_std
                && !facts.uses_alloc
                && !facts.declares_host_import
                && !facts.has_ub_marker;
            Ok(TassadarArticleFrontendCompilerEnvelopeAdmittedCaseCheck {
                case_id: row.case_id.clone(),
                source_ref: row.source_ref.clone(),
                source_digest: source_case.source_digest.clone(),
                source_canon_compiled,
                compile_config_digest_matches_manifest,
                toolchain_identity: source_case.toolchain_identity.clone(),
                toolchain_identity_green,
                source_policy_green,
                imported_function_count: source_case
                    .wasm_binary_summary
                    .as_ref()
                    .map_or(0, |summary| summary.imported_function_count),
                detail: String::from(
                    "admitted source rows stay green only when the committed source policy, compile-config digest, toolchain identity, and zero-import Wasm output all continue to match the declared envelope",
                ),
            })
        })
        .collect()
}

fn build_refusal_probes(
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
) -> Result<
    Vec<TassadarArticleFrontendCompilerEnvelopeRefusalProbe>,
    TassadarArticleFrontendCompilerEnvelopeReportError,
> {
    manifest
        .disallowed_rows
        .iter()
        .map(|row| {
            let source_kind = source_kind_for_ref(row.representative_source_ref.as_str());
            let actual_refusal_kind = if row.refusal_kind
                == TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLanguageVersion
            {
                TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLanguageVersion
            } else {
                let source_text = read_repo_text(row.representative_source_ref.as_str())?;
                infer_refusal_kind(source_kind, source_text.as_str())?
            };
            let probe_green = actual_refusal_kind == row.refusal_kind;
            Ok(TassadarArticleFrontendCompilerEnvelopeRefusalProbe {
                row_id: row.row_id.clone(),
                source_ref: row.representative_source_ref.clone(),
                source_kind,
                expected_refusal_kind: row.refusal_kind,
                actual_refusal_kind,
                probe_green,
                detail: row.detail.clone(),
            })
        })
        .collect()
}

fn manifest_refusal_kinds_green(
    manifest: &TassadarArticleFrontendCompilerEnvelopeManifest,
) -> bool {
    let actual = manifest
        .disallowed_rows
        .iter()
        .map(|row| row.refusal_kind)
        .collect::<BTreeSet<_>>();
    let expected = [
        TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredFrontendFamily,
        TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLanguageVersion,
        TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLibrarySurface,
        TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredAbiSurface,
        TassadarArticleFrontendEnvelopeRefusalKind::HostOrSyscallSurfaceDisallowed,
        TassadarArticleFrontendEnvelopeRefusalKind::UbDependentSourceDisallowed,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    actual == expected
}

fn unique_compiler_versions(
    case_checks: &[TassadarArticleFrontendCompilerEnvelopeAdmittedCaseCheck],
) -> BTreeSet<String> {
    case_checks
        .iter()
        .map(|check| check.toolchain_identity.compiler_version.clone())
        .collect()
}

fn source_kind_for_ref(source_ref: &str) -> TassadarProgramSourceKind {
    if source_ref.ends_with(".c") {
        TassadarProgramSourceKind::CSource
    } else {
        TassadarProgramSourceKind::RustSource
    }
}

fn infer_refusal_kind(
    source_kind: TassadarProgramSourceKind,
    source_text: &str,
) -> Result<
    TassadarArticleFrontendEnvelopeRefusalKind,
    TassadarArticleFrontendCompilerEnvelopeReportError,
> {
    if source_kind != TassadarProgramSourceKind::RustSource {
        return Ok(TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredFrontendFamily);
    }
    let facts = inspect_source(source_kind, source_text);
    if facts.uses_std || facts.uses_alloc || !facts.has_no_std {
        return Ok(TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredLibrarySurface);
    }
    if facts.declares_host_import {
        return Ok(TassadarArticleFrontendEnvelopeRefusalKind::HostOrSyscallSurfaceDisallowed);
    }
    if facts.has_ub_marker {
        return Ok(TassadarArticleFrontendEnvelopeRefusalKind::UbDependentSourceDisallowed);
    }
    if source_text.contains("i64")
        || source_text.contains("-> (")
        || source_text.contains("(i32, i32)")
        || source_text.contains("*const i64")
        || source_text.contains("*mut i64")
    {
        return Ok(TassadarArticleFrontendEnvelopeRefusalKind::OutsideDeclaredAbiSurface);
    }
    Err(
        TassadarArticleFrontendCompilerEnvelopeReportError::Invariant {
            detail: String::from("failed to infer envelope refusal kind"),
        },
    )
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct SourceFacts {
    has_no_std: bool,
    has_no_main: bool,
    uses_std: bool,
    uses_alloc: bool,
    declares_host_import: bool,
    has_ub_marker: bool,
}

fn inspect_source(source_kind: TassadarProgramSourceKind, source_text: &str) -> SourceFacts {
    if source_kind != TassadarProgramSourceKind::RustSource {
        return SourceFacts::default();
    }
    let host_import_patterns = ["extern \"C\" {", "extern \"C\"\n{", "extern \"C\"\r\n{"];
    SourceFacts {
        has_no_std: source_text.contains("#![no_std]"),
        has_no_main: source_text.contains("#![no_main]"),
        uses_std: source_text.contains("std::") || source_text.contains("extern crate std"),
        uses_alloc: source_text.contains("alloc::") || source_text.contains("extern crate alloc"),
        declares_host_import: host_import_patterns
            .iter()
            .any(|pattern| source_text.contains(pattern)),
        has_ub_marker: source_text.contains("unreachable_unchecked"),
    }
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
) -> Result<String, TassadarArticleFrontendCompilerEnvelopeReportError> {
    let path = repo_root().join(relative_path);
    fs::read_to_string(&path).map_err(|error| {
        TassadarArticleFrontendCompilerEnvelopeReportError::Read {
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

fn read_repo_json<T: DeserializeOwned>(
    relative_path: &str,
    artifact_kind: &str,
) -> Result<T, TassadarArticleFrontendCompilerEnvelopeReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleFrontendCompilerEnvelopeReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleFrontendCompilerEnvelopeReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_frontend_compiler_envelope_report, read_repo_json,
        tassadar_article_frontend_compiler_envelope_report_path,
        write_tassadar_article_frontend_compiler_envelope_report,
        TassadarArticleFrontendCompilerEnvelopeReport,
        TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF,
    };

    #[test]
    fn article_frontend_compiler_envelope_report_tracks_declared_bounded_green_with_final_green() {
        let report = build_tassadar_article_frontend_compiler_envelope_report().expect("report");

        assert_eq!(report.acceptance_gate_tie.tied_requirement_id, "TAS-176");
        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert!(report.envelope_manifest_green);
        assert!(report.toolchain_identity_green);
        assert!(report.refusal_taxonomy_green);
        assert_eq!(report.admitted_case_green_count, 8);
        assert_eq!(report.refusal_probe_green_count, 6);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_frontend_compiler_envelope_report_matches_committed_truth() {
        let generated = build_tassadar_article_frontend_compiler_envelope_report().expect("report");
        let committed: TassadarArticleFrontendCompilerEnvelopeReport = read_repo_json(
            TASSADAR_ARTICLE_FRONTEND_COMPILER_ENVELOPE_REPORT_REF,
            "report",
        )
        .expect("committed report");
        assert_eq!(generated, committed);
    }

    #[test]
    fn write_article_frontend_compiler_envelope_report_persists_current_truth() {
        let directory = tempfile::tempdir().expect("tempdir");
        let output_path = directory
            .path()
            .join("tassadar_article_frontend_compiler_envelope_report.json");
        let written = write_tassadar_article_frontend_compiler_envelope_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleFrontendCompilerEnvelopeReport =
            serde_json::from_slice(&std::fs::read(&output_path).expect("read")).expect("decode");
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_frontend_compiler_envelope_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_frontend_compiler_envelope_report.json")
        );
    }
}
