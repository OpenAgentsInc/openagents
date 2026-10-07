use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(test)]
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use psionic_data::{
    build_tassadar_article_interpreter_breadth_suite,
    write_tassadar_article_interpreter_breadth_suite, TassadarArticleInterpreterBreadthSuite,
    TassadarArticleInterpreterBreadthSuiteError, TassadarArticleInterpreterBreadthSuiteFamilyId,
    TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF,
};
use psionic_models::{
    TassadarRustArticleProfileCategory, TassadarRustArticleProfileCompletenessPublication,
    TassadarRustArticleProfileRowStatus,
};
use psionic_runtime::TassadarArticleRuntimeFloorStatus;

use crate::{
    build_tassadar_article_equivalence_acceptance_gate_report,
    build_tassadar_article_frontend_corpus_compile_matrix_report,
    build_tassadar_article_interpreter_breadth_envelope_report,
    build_tassadar_article_runtime_closeout_report, build_tassadar_call_frame_report,
    build_tassadar_module_scale_workload_suite_report,
    build_tassadar_rust_article_profile_completeness_report, build_tassadar_trap_exception_report,
    tassadar_article_interpreter_breadth_envelope_report_path,
    write_tassadar_article_interpreter_breadth_envelope_report,
    TassadarArticleEquivalenceAcceptanceGateReport,
    TassadarArticleEquivalenceAcceptanceGateReportError,
    TassadarArticleEquivalenceAcceptanceStatus, TassadarArticleFrontendCorpusCaseStatus,
    TassadarArticleFrontendCorpusCompileMatrixReport,
    TassadarArticleFrontendCorpusCompileMatrixReportError,
    TassadarArticleInterpreterBreadthEnvelopeReport,
    TassadarArticleInterpreterBreadthEnvelopeReportError, TassadarArticleRuntimeCloseoutReport,
    TassadarArticleRuntimeCloseoutReportError, TassadarCallFrameCaseStatus,
    TassadarCallFrameReport, TassadarCallFrameReportError, TassadarModuleScaleWorkloadSuiteReport,
    TassadarModuleScaleWorkloadSuiteReportError, TassadarTrapExceptionReport,
    TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
    TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF,
};
use psionic_data::TassadarModuleScaleWorkloadStatus;

pub const TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_interpreter_breadth_suite_gate_report.json";
pub const TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_CHECKER_REF: &str =
    "scripts/check-tassadar-article-interpreter-breadth-suite-gate.sh";

const TIED_REQUIREMENT_ID: &str = "TAS-179A";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthSuiteGateAcceptanceTie {
    pub acceptance_gate_report_ref: String,
    pub tied_requirement_id: String,
    pub tied_requirement_satisfied: bool,
    pub acceptance_status: TassadarArticleEquivalenceAcceptanceStatus,
    pub blocked_issue_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthSuiteFamilyCheck {
    pub family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    pub envelope_anchor_family_count: usize,
    pub envelope_anchor_green: bool,
    pub authority_ref_count: usize,
    pub authority_refs_exist: bool,
    pub owner_surface_ref_count: usize,
    pub owner_surface_refs_exist: bool,
    pub required_evidence_count: usize,
    pub missing_required_evidence_ids: Vec<String>,
    pub required_evidence_green: bool,
    pub evidence_detail: String,
    pub green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthSuiteGateContractCheck {
    pub suite_manifest_ref: String,
    pub envelope_report_ref: String,
    pub gate_issue_green: bool,
    pub envelope_manifest_alignment_green: bool,
    pub required_family_count_green: bool,
    pub family_row_count_green: bool,
    pub family_alignment_green: bool,
    pub authority_and_owner_refs_green: bool,
    pub required_evidence_green: bool,
    pub contract_green: bool,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthSuiteGateReport {
    pub schema_version: u16,
    pub report_id: String,
    pub checker_script_ref: String,
    pub acceptance_gate_tie: TassadarArticleInterpreterBreadthSuiteGateAcceptanceTie,
    pub envelope_report_ref: String,
    pub envelope_report: TassadarArticleInterpreterBreadthEnvelopeReport,
    pub suite_manifest_ref: String,
    pub suite_manifest: TassadarArticleInterpreterBreadthSuite,
    pub contract_check: TassadarArticleInterpreterBreadthSuiteGateContractCheck,
    pub family_checks: Vec<TassadarArticleInterpreterBreadthSuiteFamilyCheck>,
    pub green_family_count: usize,
    pub breadth_gate_green: bool,
    pub article_equivalence_green: bool,
    pub claim_boundary: String,
    pub summary: String,
    pub report_digest: String,
}

#[derive(Debug, Error)]
pub enum TassadarArticleInterpreterBreadthSuiteGateReportError {
    #[error(transparent)]
    AcceptanceGate(#[from] TassadarArticleEquivalenceAcceptanceGateReportError),
    #[error(transparent)]
    Envelope(#[from] TassadarArticleInterpreterBreadthEnvelopeReportError),
    #[error(transparent)]
    SuiteManifest(#[from] TassadarArticleInterpreterBreadthSuiteError),
    #[error(transparent)]
    FrontendCorpus(#[from] TassadarArticleFrontendCorpusCompileMatrixReportError),
    #[error(transparent)]
    CallFrames(#[from] TassadarCallFrameReportError),
    #[error(transparent)]
    RuntimeCloseout(#[from] TassadarArticleRuntimeCloseoutReportError),
    #[error(transparent)]
    ModuleScale(#[from] TassadarModuleScaleWorkloadSuiteReportError),
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

struct EvidenceInputs<'a> {
    frontend_corpus: &'a TassadarArticleFrontendCorpusCompileMatrixReport,
    call_frame: &'a TassadarCallFrameReport,
    rust_profile: &'a TassadarRustArticleProfileCompletenessPublication,
    runtime_closeout: &'a TassadarArticleRuntimeCloseoutReport,
    module_scale: &'a TassadarModuleScaleWorkloadSuiteReport,
    trap_exception: &'a TassadarTrapExceptionReport,
}

pub fn build_tassadar_article_interpreter_breadth_suite_gate_report() -> Result<
    TassadarArticleInterpreterBreadthSuiteGateReport,
    TassadarArticleInterpreterBreadthSuiteGateReportError,
> {
    let acceptance_gate = build_tassadar_article_equivalence_acceptance_gate_report()?;
    let envelope_report = build_tassadar_article_interpreter_breadth_envelope_report()?;
    let suite_manifest = build_tassadar_article_interpreter_breadth_suite();
    let frontend_corpus = build_tassadar_article_frontend_corpus_compile_matrix_report()?;
    let call_frame = build_tassadar_call_frame_report()?;
    let rust_profile = build_tassadar_rust_article_profile_completeness_report();
    let runtime_closeout = build_tassadar_article_runtime_closeout_report()?;
    let module_scale = build_tassadar_module_scale_workload_suite_report()?;
    let trap_exception = build_tassadar_trap_exception_report();
    Ok(build_report_from_inputs(
        acceptance_gate,
        envelope_report,
        suite_manifest,
        EvidenceInputs {
            frontend_corpus: &frontend_corpus,
            call_frame: &call_frame,
            rust_profile: &rust_profile,
            runtime_closeout: &runtime_closeout,
            module_scale: &module_scale,
            trap_exception: &trap_exception,
        },
    ))
}

fn build_report_from_inputs(
    acceptance_gate: TassadarArticleEquivalenceAcceptanceGateReport,
    envelope_report: TassadarArticleInterpreterBreadthEnvelopeReport,
    suite_manifest: TassadarArticleInterpreterBreadthSuite,
    evidence: EvidenceInputs<'_>,
) -> TassadarArticleInterpreterBreadthSuiteGateReport {
    let acceptance_gate_tie = TassadarArticleInterpreterBreadthSuiteGateAcceptanceTie {
        acceptance_gate_report_ref: String::from(
            TASSADAR_ARTICLE_EQUIVALENCE_ACCEPTANCE_GATE_REPORT_REF,
        ),
        tied_requirement_id: String::from(TIED_REQUIREMENT_ID),
        tied_requirement_satisfied: acceptance_gate
            .green_requirement_ids
            .iter()
            .any(|id| id == TIED_REQUIREMENT_ID),
        acceptance_status: acceptance_gate.acceptance_status,
        blocked_issue_ids: acceptance_gate.blocked_issue_ids.clone(),
    };
    let family_checks = suite_manifest
        .family_rows
        .iter()
        .map(|row| build_family_check(row, &envelope_report, &evidence))
        .collect::<Vec<_>>();
    let contract_check =
        build_contract_check(&suite_manifest, &envelope_report, family_checks.as_slice());
    let green_family_count = family_checks.iter().filter(|check| check.green).count();
    let breadth_gate_green = acceptance_gate_tie.tied_requirement_satisfied
        && envelope_report.envelope_contract_green
        && contract_check.contract_green
        && green_family_count == suite_manifest.required_family_ids.len();
    let mut report = TassadarArticleInterpreterBreadthSuiteGateReport {
        schema_version: 1,
        report_id: String::from("tassadar.article_interpreter_breadth_suite_gate.report.v1"),
        checker_script_ref: String::from(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_CHECKER_REF,
        ),
        acceptance_gate_tie: acceptance_gate_tie.clone(),
        envelope_report_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF),
        envelope_report,
        suite_manifest_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF),
        suite_manifest,
        contract_check,
        family_checks,
        green_family_count,
        breadth_gate_green,
        article_equivalence_green: acceptance_gate_tie.blocked_issue_ids.is_empty()
            && breadth_gate_green,
        claim_boundary: String::from(
            "this report closes TAS-179A only. It proves the declared TAS-179 interpreter-breadth envelope over one generic article-program family suite spanning arithmetic, call-heavy, allocator-backed, indirect-call, branch-heavy, loop-heavy, state-machine, and parser-style rows. It still does not imply arbitrary-program closure, benchmark-wide article closure, or final article-equivalence green status.",
        ),
        summary: String::new(),
        report_digest: String::new(),
    };
    report.summary = format!(
        "Article interpreter breadth suite gate now records tied_requirement_satisfied={}, green_families={}/{}, breadth_gate_green={}, blocked_issue_frontier={:?}, and article_equivalence_green={}.",
        report.acceptance_gate_tie.tied_requirement_satisfied,
        report.green_family_count,
        report.suite_manifest.required_family_ids.len(),
        report.breadth_gate_green,
        report.acceptance_gate_tie.blocked_issue_ids.first(),
        report.article_equivalence_green,
    );
    report.report_digest = stable_digest(
        b"psionic_tassadar_article_interpreter_breadth_suite_gate_report|",
        &report,
    );
    report
}

fn build_family_check(
    row: &psionic_data::TassadarArticleInterpreterBreadthSuiteRow,
    envelope_report: &TassadarArticleInterpreterBreadthEnvelopeReport,
    evidence: &EvidenceInputs<'_>,
) -> TassadarArticleInterpreterBreadthSuiteFamilyCheck {
    let authority_refs_exist = row
        .authority_refs
        .iter()
        .all(|path| repo_root().join(path).is_file());
    let owner_surface_refs_exist = row
        .owner_surface_refs
        .iter()
        .all(|path| repo_root().join(path).is_file());
    let envelope_anchor_green = row.envelope_anchor_family_ids.iter().all(|family_id| {
        envelope_report
            .manifest
            .current_floor_family_ids
            .contains(family_id)
            || envelope_report
                .manifest
                .declared_required_family_ids
                .contains(family_id)
    });
    let missing_required_evidence_ids = row
        .required_evidence_ids
        .iter()
        .filter(|required_evidence_id| {
            !required_evidence_present(row.family_id, required_evidence_id.as_str(), evidence)
        })
        .cloned()
        .collect::<Vec<_>>();
    let required_evidence_green = missing_required_evidence_ids.is_empty();
    let green = authority_refs_exist
        && owner_surface_refs_exist
        && envelope_anchor_green
        && required_evidence_green;
    TassadarArticleInterpreterBreadthSuiteFamilyCheck {
        family_id: row.family_id,
        envelope_anchor_family_count: row.envelope_anchor_family_ids.len(),
        envelope_anchor_green,
        authority_ref_count: row.authority_refs.len(),
        authority_refs_exist,
        owner_surface_ref_count: row.owner_surface_refs.len(),
        owner_surface_refs_exist,
        required_evidence_count: row.required_evidence_ids.len(),
        missing_required_evidence_ids,
        required_evidence_green,
        evidence_detail: evidence_detail_for_family(row.family_id),
        green,
        detail: row.detail.clone(),
    }
}

fn build_contract_check(
    suite_manifest: &TassadarArticleInterpreterBreadthSuite,
    envelope_report: &TassadarArticleInterpreterBreadthEnvelopeReport,
    family_checks: &[TassadarArticleInterpreterBreadthSuiteFamilyCheck],
) -> TassadarArticleInterpreterBreadthSuiteGateContractCheck {
    let gate_issue_green = suite_manifest.gate_issue_id == "TAS-179A";
    let envelope_manifest_alignment_green =
        suite_manifest.envelope_manifest_ref == envelope_report.manifest_ref;
    let required_family_count_green = suite_manifest.required_family_ids.len() == 8;
    let family_row_count_green = suite_manifest.family_rows.len() == 8;
    let family_alignment_green = family_checks
        .iter()
        .all(|check| check.envelope_anchor_green);
    let authority_and_owner_refs_green = family_checks
        .iter()
        .all(|check| check.authority_refs_exist && check.owner_surface_refs_exist);
    let required_evidence_green = family_checks
        .iter()
        .all(|check| check.required_evidence_green);
    let contract_green = gate_issue_green
        && envelope_manifest_alignment_green
        && required_family_count_green
        && family_row_count_green
        && family_alignment_green
        && authority_and_owner_refs_green
        && required_evidence_green;
    TassadarArticleInterpreterBreadthSuiteGateContractCheck {
        suite_manifest_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF),
        envelope_report_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REPORT_REF),
        gate_issue_green,
        envelope_manifest_alignment_green,
        required_family_count_green,
        family_row_count_green,
        family_alignment_green,
        authority_and_owner_refs_green,
        required_evidence_green,
        contract_green,
        detail: String::from(
            "the TAS-179A suite gate must stay tied to the TAS-179 envelope, keep the exact eight required generic article-program families fixed, preserve concrete authority refs and owner surfaces, and keep every declared evidence row mechanically green.",
        ),
    }
}

fn required_evidence_present(
    family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    required_evidence_id: &str,
    evidence: &EvidenceInputs<'_>,
) -> bool {
    match family_id {
        TassadarArticleInterpreterBreadthSuiteFamilyId::ArithmeticPrograms => match required_evidence_id
        {
            "arithmetic_accumulator_exact" => evidence.frontend_corpus.case_rows.iter().any(|row| {
                row.case_id == "arithmetic_accumulator_exact"
                    && row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled
                    && row.lineage_green
            }),
            "arithmetic_reference_success" => evidence
                .trap_exception
                .case_audits
                .iter()
                .any(|case| case.case_id == "arithmetic_reference_success" && case.parity_preserved),
            _ => false,
        },
        TassadarArticleInterpreterBreadthSuiteFamilyId::CallHeavyPrograms => {
            match required_evidence_id {
                "multi_function_replay" | "bounded_recursive_exact" => evidence
                    .call_frame
                    .cases
                    .iter()
                    .any(|case| {
                        case.case_id == required_evidence_id
                            && case.status == TassadarCallFrameCaseStatus::Exact
                    }),
                "bounded_recursion_refusal" => evidence.call_frame.cases.iter().any(|case| {
                    case.case_id == "bounded_recursion_refusal"
                        && case.status == TassadarCallFrameCaseStatus::Refused
                        && case.refusal_kind.as_deref() == Some("recursion_depth_exceeded")
                }),
                _ => false,
            }
        }
        TassadarArticleInterpreterBreadthSuiteFamilyId::AllocatorBackedPrograms => {
            matches!(
                required_evidence_id,
                "bump_allocator_checksum_exact" | "heap_sum_window_exact"
            ) && evidence.frontend_corpus.case_rows.iter().any(|row| {
                row.case_id == required_evidence_id
                    && row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled
                    && row.lineage_green
            })
        }
        TassadarArticleInterpreterBreadthSuiteFamilyId::IndirectCallPrograms => {
            match required_evidence_id {
                "tables_globals_indirect_calls.single_funcref_table_mutable_i32_globals" => evidence
                    .rust_profile
                    .rows
                    .iter()
                    .any(|row| {
                        row.row_id
                            == "tables_globals_indirect_calls.single_funcref_table_mutable_i32_globals"
                            && row.category
                                == TassadarRustArticleProfileCategory::TableGlobalIndirectCallShape
                            && row.status == TassadarRustArticleProfileRowStatus::Supported
                    }),
                "sudoku_indirect_call_failure" => evidence.trap_exception.case_audits.iter().any(
                    |case| {
                        case.case_id == "sudoku_indirect_call_failure"
                            && case.observed_non_success_kind.as_deref()
                                == Some("indirect_call_failure")
                            && case.parity_preserved
                    },
                ),
                _ => false,
            }
        }
        TassadarArticleInterpreterBreadthSuiteFamilyId::BranchHeavyPrograms => {
            required_evidence_id == "branch_dispatch_exact"
                && evidence.frontend_corpus.case_rows.iter().any(|row| {
                    row.case_id == "branch_dispatch_exact"
                        && row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled
                        && row.lineage_green
                })
        }
        TassadarArticleInterpreterBreadthSuiteFamilyId::LoopHeavyPrograms => match required_evidence_id
        {
            "long_loop_frontier_exact" => evidence.frontend_corpus.case_rows.iter().any(|row| {
                row.case_id == "long_loop_frontier_exact"
                    && row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled
                    && row.lineage_green
            }),
            "long_loop_kernel.million_step" => runtime_horizon_present(
                evidence.runtime_closeout,
                "long_loop_kernel.million_step",
            ),
            _ => false,
        },
        TassadarArticleInterpreterBreadthSuiteFamilyId::StateMachinePrograms => {
            match required_evidence_id {
                "state_machine_router_exact" | "state_machine_loop_exact" => evidence
                    .frontend_corpus
                    .case_rows
                    .iter()
                    .any(|row| {
                        row.case_id == required_evidence_id
                            && row.actual_status == TassadarArticleFrontendCorpusCaseStatus::Compiled
                            && row.lineage_green
                    }),
                "state_machine_kernel.two_million_step" => runtime_horizon_present(
                    evidence.runtime_closeout,
                    "state_machine_kernel.two_million_step",
                ),
                _ => false,
            }
        }
        TassadarArticleInterpreterBreadthSuiteFamilyId::ParserStylePrograms => {
            required_evidence_id == "parsing_token_triplet_exact"
                && evidence.module_scale.cases.iter().any(|case| {
                    case.case_id == "parsing_token_triplet_exact"
                        && case.status == TassadarModuleScaleWorkloadStatus::LoweredExact
                        && case.exactness_bps == Some(10_000)
                })
        }
    }
}

fn runtime_horizon_present(
    runtime_closeout: &TassadarArticleRuntimeCloseoutReport,
    horizon_id: &str,
) -> bool {
    runtime_closeout
        .bundle
        .horizon_receipts
        .iter()
        .any(|receipt| {
            receipt.horizon_id == horizon_id
                && receipt.floor_status == TassadarArticleRuntimeFloorStatus::Passed
                && receipt.exactness_bps == 10_000
        })
}

fn evidence_detail_for_family(family_id: TassadarArticleInterpreterBreadthSuiteFamilyId) -> String {
    match family_id {
        TassadarArticleInterpreterBreadthSuiteFamilyId::ArithmeticPrograms => String::from(
            "arithmetic stays green through the committed TAS-177 article frontend corpus row plus the explicit trap/exception arithmetic success control row",
        ),
        TassadarArticleInterpreterBreadthSuiteFamilyId::CallHeavyPrograms => String::from(
            "call-heavy stays green through exact multi-function replay, exact bounded recursion, and explicit recursion-cap refusal in the call-frame lane",
        ),
        TassadarArticleInterpreterBreadthSuiteFamilyId::AllocatorBackedPrograms => String::from(
            "allocator-backed stays green through the committed bump-allocator and heap-window article corpus rows",
        ),
        TassadarArticleInterpreterBreadthSuiteFamilyId::IndirectCallPrograms => String::from(
            "indirect-call stays green through the admitted single-table zero-parameter profile row plus explicit indirect-call trap parity in the search-heavy trap audit",
        ),
        TassadarArticleInterpreterBreadthSuiteFamilyId::BranchHeavyPrograms => String::from(
            "branch-heavy stays green through the committed branch-dispatch article corpus row",
        ),
        TassadarArticleInterpreterBreadthSuiteFamilyId::LoopHeavyPrograms => String::from(
            "loop-heavy stays green through the compiled long-loop article corpus row plus the passed runtime million-step horizon receipt",
        ),
        TassadarArticleInterpreterBreadthSuiteFamilyId::StateMachinePrograms => String::from(
            "state-machine stays green through the committed router and loop corpus rows plus the passed two-million-step runtime horizon receipt",
        ),
        TassadarArticleInterpreterBreadthSuiteFamilyId::ParserStylePrograms => String::from(
            "parser-style stays green through the bounded module-scale parsing fixture rather than linked-program-bundle spillover",
        ),
    }
}

pub fn tassadar_article_interpreter_breadth_suite_gate_report_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF)
}

pub fn write_tassadar_article_interpreter_breadth_suite_gate_report(
    output_path: impl AsRef<Path>,
) -> Result<
    TassadarArticleInterpreterBreadthSuiteGateReport,
    TassadarArticleInterpreterBreadthSuiteGateReportError,
> {
    write_tassadar_article_interpreter_breadth_suite(
        repo_root().join(TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF),
    )?;
    write_tassadar_article_interpreter_breadth_envelope_report(
        tassadar_article_interpreter_breadth_envelope_report_path(),
    )?;
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleInterpreterBreadthSuiteGateReportError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let report = build_tassadar_article_interpreter_breadth_suite_gate_report()?;
    let json = serde_json::to_string_pretty(&report)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterBreadthSuiteGateReportError::Write {
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
) -> Result<T, TassadarArticleInterpreterBreadthSuiteGateReportError> {
    let path = repo_root().join(relative_path);
    let bytes = fs::read(&path).map_err(|error| {
        TassadarArticleInterpreterBreadthSuiteGateReportError::Read {
            path: path.display().to_string(),
            error,
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        TassadarArticleInterpreterBreadthSuiteGateReportError::Decode {
            path: format!("{} ({artifact_kind})", path.display()),
            error,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_interpreter_breadth_suite_gate_report, read_repo_json,
        tassadar_article_interpreter_breadth_suite_gate_report_path,
        write_tassadar_article_interpreter_breadth_suite_gate_report,
        TassadarArticleInterpreterBreadthSuiteGateReport,
        TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
    };

    #[test]
    fn article_interpreter_breadth_suite_gate_closes_breadth_with_final_article_equivalence() {
        let report =
            build_tassadar_article_interpreter_breadth_suite_gate_report().expect("report");

        assert!(report.acceptance_gate_tie.tied_requirement_satisfied);
        assert!(report.acceptance_gate_tie.blocked_issue_ids.is_empty());
        assert_eq!(report.green_family_count, 8);
        assert!(report.contract_check.contract_green);
        assert!(report.breadth_gate_green);
        assert!(report.article_equivalence_green);
    }

    #[test]
    fn article_interpreter_breadth_suite_gate_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated =
            build_tassadar_article_interpreter_breadth_suite_gate_report().expect("report");
        let committed: TassadarArticleInterpreterBreadthSuiteGateReport = read_repo_json(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_GATE_REPORT_REF,
            "article_interpreter_breadth_suite_gate_report",
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_interpreter_breadth_suite_gate_report_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let output_path = directory
            .path()
            .join("tassadar_article_interpreter_breadth_suite_gate_report.json");
        let written = write_tassadar_article_interpreter_breadth_suite_gate_report(&output_path)
            .expect("write report");
        let persisted: TassadarArticleInterpreterBreadthSuiteGateReport =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_interpreter_breadth_suite_gate_report_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_interpreter_breadth_suite_gate_report.json")
        );
        Ok(())
    }
}
