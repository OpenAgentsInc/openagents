use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{
    TassadarArticleInterpreterFamilyId, TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF,
};

pub const TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF: &str =
    "fixtures/tassadar/sources/tassadar_article_interpreter_breadth_suite_v1.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleInterpreterBreadthSuiteFamilyId {
    ArithmeticPrograms,
    CallHeavyPrograms,
    AllocatorBackedPrograms,
    IndirectCallPrograms,
    BranchHeavyPrograms,
    LoopHeavyPrograms,
    StateMachinePrograms,
    ParserStylePrograms,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthSuiteRow {
    pub family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    pub envelope_anchor_family_ids: Vec<TassadarArticleInterpreterFamilyId>,
    pub authority_refs: Vec<String>,
    pub owner_surface_refs: Vec<String>,
    pub required_evidence_ids: Vec<String>,
    pub detail: String,
}

impl TassadarArticleInterpreterBreadthSuiteRow {
    fn validate(&self) -> Result<(), TassadarArticleInterpreterBreadthSuiteError> {
        if self.envelope_anchor_family_ids.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthSuiteError::MissingEnvelopeAnchorFamilies {
                    family_id: self.family_id,
                },
            );
        }
        if self.authority_refs.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthSuiteError::MissingAuthorityRefs {
                    family_id: self.family_id,
                },
            );
        }
        if self.owner_surface_refs.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthSuiteError::MissingOwnerSurfaceRefs {
                    family_id: self.family_id,
                },
            );
        }
        if self.required_evidence_ids.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthSuiteError::MissingRequiredEvidenceIds {
                    family_id: self.family_id,
                },
            );
        }
        if self
            .authority_refs
            .iter()
            .any(|authority_ref| authority_ref.trim().is_empty())
        {
            return Err(
                TassadarArticleInterpreterBreadthSuiteError::InvalidAuthorityRef {
                    family_id: self.family_id,
                },
            );
        }
        if self
            .owner_surface_refs
            .iter()
            .any(|owner_surface_ref| owner_surface_ref.trim().is_empty())
        {
            return Err(
                TassadarArticleInterpreterBreadthSuiteError::InvalidOwnerSurfaceRef {
                    family_id: self.family_id,
                },
            );
        }
        if self
            .required_evidence_ids
            .iter()
            .any(|required_evidence_id| required_evidence_id.trim().is_empty())
        {
            return Err(
                TassadarArticleInterpreterBreadthSuiteError::InvalidRequiredEvidenceId {
                    family_id: self.family_id,
                },
            );
        }
        if self.detail.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingDetail {
                family_id: self.family_id,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthSuite {
    pub schema_version: u16,
    pub manifest_id: String,
    pub manifest_ref: String,
    pub envelope_manifest_ref: String,
    pub gate_issue_id: String,
    pub gate_report_ref: String,
    pub gate_summary_ref: String,
    pub required_family_ids: Vec<TassadarArticleInterpreterBreadthSuiteFamilyId>,
    pub family_rows: Vec<TassadarArticleInterpreterBreadthSuiteRow>,
    pub current_truth_boundary: String,
    pub non_implications: Vec<String>,
    pub claim_boundary: String,
    pub manifest_digest: String,
}

impl TassadarArticleInterpreterBreadthSuite {
    fn new() -> Self {
        let mut manifest = Self {
            schema_version: 1,
            manifest_id: String::from("tassadar.article_interpreter_breadth_suite.v1"),
            manifest_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF),
            envelope_manifest_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF),
            gate_issue_id: String::from("TAS-179A"),
            gate_report_ref: String::from(
                "fixtures/tassadar/reports/tassadar_article_interpreter_breadth_suite_gate_report.json",
            ),
            gate_summary_ref: String::from(
                "fixtures/tassadar/reports/tassadar_article_interpreter_breadth_suite_gate_summary.json",
            ),
            required_family_ids: vec![
                TassadarArticleInterpreterBreadthSuiteFamilyId::ArithmeticPrograms,
                TassadarArticleInterpreterBreadthSuiteFamilyId::CallHeavyPrograms,
                TassadarArticleInterpreterBreadthSuiteFamilyId::AllocatorBackedPrograms,
                TassadarArticleInterpreterBreadthSuiteFamilyId::IndirectCallPrograms,
                TassadarArticleInterpreterBreadthSuiteFamilyId::BranchHeavyPrograms,
                TassadarArticleInterpreterBreadthSuiteFamilyId::LoopHeavyPrograms,
                TassadarArticleInterpreterBreadthSuiteFamilyId::StateMachinePrograms,
                TassadarArticleInterpreterBreadthSuiteFamilyId::ParserStylePrograms,
            ],
            family_rows: family_rows(),
            current_truth_boundary: String::from(
                "the public repo now freezes one generic article-program family suite over arithmetic, call-heavy, allocator-backed, indirect-call, branch-heavy, loop-heavy, state-machine, and parser-style rows. Each row stays tied to the declared TAS-179 interpreter envelope, concrete authority refs, and explicit owner surfaces rather than borrowing arbitrary-program language by implication.",
            ),
            non_implications: vec![
                String::from("not arbitrary-program closure inside transformer weights"),
                String::from("not arbitrary host-import or OS-mediated process closure"),
                String::from("not memory64, multi-memory, component-linking, or exception-profile closure"),
                String::from("not benchmark-wide Hungarian or Sudoku closeout"),
                String::from("not final article-equivalence green status"),
            ],
            claim_boundary: String::from(
                "this manifest defines only the required generic article-program family suite for TAS-179A. A green suite gate proves the declared breadth rows are covered by committed evidence under the TAS-179 envelope; it does not widen the article claim to arbitrary programs or final article-equivalence closure.",
            ),
            manifest_digest: String::new(),
        };
        manifest
            .validate()
            .expect("article interpreter breadth suite should validate");
        manifest.manifest_digest = stable_digest(
            b"psionic_tassadar_article_interpreter_breadth_suite|",
            &manifest,
        );
        manifest
    }

    pub fn validate(&self) -> Result<(), TassadarArticleInterpreterBreadthSuiteError> {
        if self.manifest_id.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingManifestId);
        }
        if self.manifest_ref.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingManifestRef);
        }
        if self.envelope_manifest_ref.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingEnvelopeManifestRef);
        }
        if self.gate_issue_id.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingGateIssueId);
        }
        if self.gate_report_ref.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingGateReportRef);
        }
        if self.gate_summary_ref.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingGateSummaryRef);
        }
        if self.required_family_ids.is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingRequiredFamilies);
        }
        if self.family_rows.is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingFamilyRows);
        }
        if self.current_truth_boundary.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingCurrentTruthBoundary);
        }
        if self.non_implications.is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingNonImplications);
        }
        if self
            .non_implications
            .iter()
            .any(|non_implication| non_implication.trim().is_empty())
        {
            return Err(TassadarArticleInterpreterBreadthSuiteError::InvalidNonImplication);
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::MissingClaimBoundary);
        }

        let required_family_ids = self
            .required_family_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        if required_family_ids.len() != self.required_family_ids.len() {
            return Err(TassadarArticleInterpreterBreadthSuiteError::DuplicateRequiredFamily);
        }

        let mut seen_rows = BTreeSet::new();
        for row in &self.family_rows {
            row.validate()?;
            if !seen_rows.insert(row.family_id) {
                return Err(
                    TassadarArticleInterpreterBreadthSuiteError::DuplicateFamilyRow {
                        family_id: row.family_id,
                    },
                );
            }
        }

        if required_family_ids != seen_rows {
            return Err(TassadarArticleInterpreterBreadthSuiteError::FamilyRowMismatch);
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleInterpreterBreadthSuiteError {
    #[error("article interpreter breadth suite is missing `manifest_id`")]
    MissingManifestId,
    #[error("article interpreter breadth suite is missing `manifest_ref`")]
    MissingManifestRef,
    #[error("article interpreter breadth suite is missing `envelope_manifest_ref`")]
    MissingEnvelopeManifestRef,
    #[error("article interpreter breadth suite is missing `gate_issue_id`")]
    MissingGateIssueId,
    #[error("article interpreter breadth suite is missing `gate_report_ref`")]
    MissingGateReportRef,
    #[error("article interpreter breadth suite is missing `gate_summary_ref`")]
    MissingGateSummaryRef,
    #[error("article interpreter breadth suite is missing `required_family_ids`")]
    MissingRequiredFamilies,
    #[error("article interpreter breadth suite has duplicate required families")]
    DuplicateRequiredFamily,
    #[error("article interpreter breadth suite is missing `family_rows`")]
    MissingFamilyRows,
    #[error("article interpreter breadth suite row mismatch between required ids and rows")]
    FamilyRowMismatch,
    #[error(
        "article interpreter breadth suite row `{family_id:?}` is missing envelope anchor families"
    )]
    MissingEnvelopeAnchorFamilies {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error("article interpreter breadth suite row `{family_id:?}` is missing authority refs")]
    MissingAuthorityRefs {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error("article interpreter breadth suite row `{family_id:?}` is missing owner surface refs")]
    MissingOwnerSurfaceRefs {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error(
        "article interpreter breadth suite row `{family_id:?}` is missing required evidence ids"
    )]
    MissingRequiredEvidenceIds {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error("article interpreter breadth suite row `{family_id:?}` has an invalid authority ref")]
    InvalidAuthorityRef {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error(
        "article interpreter breadth suite row `{family_id:?}` has an invalid owner surface ref"
    )]
    InvalidOwnerSurfaceRef {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error(
        "article interpreter breadth suite row `{family_id:?}` has an invalid required evidence id"
    )]
    InvalidRequiredEvidenceId {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error("article interpreter breadth suite row `{family_id:?}` is missing detail")]
    MissingDetail {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error("article interpreter breadth suite has duplicate row `{family_id:?}`")]
    DuplicateFamilyRow {
        family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    },
    #[error("article interpreter breadth suite is missing `current_truth_boundary`")]
    MissingCurrentTruthBoundary,
    #[error("article interpreter breadth suite is missing `non_implications`")]
    MissingNonImplications,
    #[error("article interpreter breadth suite contains an invalid `non_implications` entry")]
    InvalidNonImplication,
    #[error("article interpreter breadth suite is missing `claim_boundary`")]
    MissingClaimBoundary,
    #[error("failed to create directory `{path}`: {error}")]
    CreateDir { path: String, error: std::io::Error },
    #[error("failed to write `{path}`: {error}")]
    Write { path: String, error: std::io::Error },
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub fn build_tassadar_article_interpreter_breadth_suite() -> TassadarArticleInterpreterBreadthSuite
{
    TassadarArticleInterpreterBreadthSuite::new()
}

pub fn tassadar_article_interpreter_breadth_suite_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF)
}

pub fn write_tassadar_article_interpreter_breadth_suite(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleInterpreterBreadthSuite, TassadarArticleInterpreterBreadthSuiteError> {
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleInterpreterBreadthSuiteError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let manifest = build_tassadar_article_interpreter_breadth_suite();
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterBreadthSuiteError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(manifest)
}

fn family_rows() -> Vec<TassadarArticleInterpreterBreadthSuiteRow> {
    vec![
        row(
            TassadarArticleInterpreterBreadthSuiteFamilyId::ArithmeticPrograms,
            &[
                TassadarArticleInterpreterFamilyId::FrozenCoreWasmWindow,
                TassadarArticleInterpreterFamilyId::ArticleNamedI32Profiles,
            ],
            &[
                "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json",
                "fixtures/tassadar/reports/tassadar_trap_exception_report.json",
            ],
            &[
                "crates/psionic-compiler/src/tassadar_article_frontend_compiler_envelope.rs",
                "crates/psionic-runtime/src/tassadar.rs",
            ],
            &[
                "arithmetic_accumulator_exact",
                "arithmetic_reference_success",
            ],
            "arithmetic rows stay pinned to the frozen/core current article profile floor instead of being smuggled in later as a generic proxy for arbitrary programs",
        ),
        row(
            TassadarArticleInterpreterBreadthSuiteFamilyId::CallHeavyPrograms,
            &[TassadarArticleInterpreterFamilyId::ArticleNamedI32Profiles],
            &["fixtures/tassadar/reports/tassadar_call_frame_report.json"],
            &[
                "crates/psionic-runtime/src/tassadar_call_frames.rs",
                "crates/psionic-runtime/src/tassadar_call_frame_resume.rs",
            ],
            &[
                "multi_function_replay",
                "bounded_recursive_exact",
                "bounded_recursion_refusal",
            ],
            "call-heavy rows must keep exact multi-function replay and bounded recursion refusal explicit rather than claiming an unconstrained call-stack model",
        ),
        row(
            TassadarArticleInterpreterBreadthSuiteFamilyId::AllocatorBackedPrograms,
            &[
                TassadarArticleInterpreterFamilyId::ArticleNamedI32Profiles,
                TassadarArticleInterpreterFamilyId::SearchProcessFamily,
            ],
            &[
                "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json",
            ],
            &[
                "crates/psionic-compiler/src/tassadar_article_frontend_compiler_envelope.rs",
                "crates/psionic-runtime/src/tassadar_article_abi.rs",
            ],
            &[
                "bump_allocator_checksum_exact",
                "heap_sum_window_exact",
            ],
            "allocator-backed rows stay tied to the committed article corpus and bounded heap-input ABI rather than widening into generic dynamic-memory closure",
        ),
        row(
            TassadarArticleInterpreterBreadthSuiteFamilyId::IndirectCallPrograms,
            &[
                TassadarArticleInterpreterFamilyId::ArticleNamedI32Profiles,
                TassadarArticleInterpreterFamilyId::SearchProcessFamily,
            ],
            &[
                "fixtures/tassadar/reports/tassadar_rust_article_profile_completeness_report.json",
                "fixtures/tassadar/reports/tassadar_trap_exception_report.json",
            ],
            &[
                "crates/psionic-compiler/src/tassadar_wasm_module.rs",
                "crates/psionic-runtime/src/tassadar_module_execution.rs",
            ],
            &[
                "tables_globals_indirect_calls.single_funcref_table_mutable_i32_globals",
                "sudoku_indirect_call_failure",
            ],
            "indirect-call rows must keep the admitted table/global shape and the challengeable indirect-call failure path explicit rather than collapsing them into generic search success",
        ),
        row(
            TassadarArticleInterpreterBreadthSuiteFamilyId::BranchHeavyPrograms,
            &[TassadarArticleInterpreterFamilyId::ArticleNamedI32Profiles],
            &[
                "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json",
            ],
            &[
                "crates/psionic-compiler/src/tassadar_article_frontend_compiler_envelope.rs",
                "crates/psionic-runtime/src/tassadar.rs",
            ],
            &["branch_dispatch_exact"],
            "branch-heavy rows stay tied to one committed Rust article source family instead of being inferred from long-loop or search evidence",
        ),
        row(
            TassadarArticleInterpreterBreadthSuiteFamilyId::LoopHeavyPrograms,
            &[TassadarArticleInterpreterFamilyId::LongHorizonControlFamily],
            &[
                "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json",
                "fixtures/tassadar/reports/tassadar_article_runtime_closeout_report.json",
            ],
            &[
                "crates/psionic-runtime/src/tassadar_article_runtime_closeout.rs",
                "crates/psionic-runtime/src/tassadar.rs",
            ],
            &[
                "long_loop_frontier_exact",
                "long_loop_kernel.million_step",
            ],
            "loop-heavy rows must keep the compiled long-loop source and the runtime stress horizon tied together instead of treating either one alone as a sufficient breadth proof",
        ),
        row(
            TassadarArticleInterpreterBreadthSuiteFamilyId::StateMachinePrograms,
            &[TassadarArticleInterpreterFamilyId::LongHorizonControlFamily],
            &[
                "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json",
                "fixtures/tassadar/reports/tassadar_article_runtime_closeout_report.json",
            ],
            &[
                "crates/psionic-runtime/src/tassadar_article_runtime_closeout.rs",
                "crates/psionic-runtime/src/tassadar.rs",
            ],
            &[
                "state_machine_router_exact",
                "state_machine_loop_exact",
                "state_machine_kernel.two_million_step",
            ],
            "state-machine rows must keep both the frontend corpus and the later runtime horizon explicit rather than letting one bounded state machine stand in for the entire long-horizon family",
        ),
        row(
            TassadarArticleInterpreterBreadthSuiteFamilyId::ParserStylePrograms,
            &[TassadarArticleInterpreterFamilyId::ModuleScaleWasmLoopFamily],
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
            ],
            &[
                "crates/psionic-ir/src/tassadar_wasm_module.rs",
                "crates/psionic-compiler/src/tassadar_wasm_module.rs",
                "crates/psionic-runtime/src/tassadar_module_execution.rs",
            ],
            &["parsing_token_triplet_exact"],
            "parser-style rows stay tied to the bounded module-scale Wasm parsing fixture rather than borrowing linked-program-bundle or broader install-time parser claims",
        ),
    ]
}

fn row(
    family_id: TassadarArticleInterpreterBreadthSuiteFamilyId,
    envelope_anchor_family_ids: &[TassadarArticleInterpreterFamilyId],
    authority_refs: &[&str],
    owner_surface_refs: &[&str],
    required_evidence_ids: &[&str],
    detail: &str,
) -> TassadarArticleInterpreterBreadthSuiteRow {
    TassadarArticleInterpreterBreadthSuiteRow {
        family_id,
        envelope_anchor_family_ids: envelope_anchor_family_ids.to_vec(),
        authority_refs: authority_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        owner_surface_refs: owner_surface_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        required_evidence_ids: required_evidence_ids
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        detail: String::from(detail),
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("psionic-data should live under <repo>/crates/psionic-data")
        .to_path_buf()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tassadar_article_interpreter_breadth_suite,
        tassadar_article_interpreter_breadth_suite_path,
        write_tassadar_article_interpreter_breadth_suite, TassadarArticleInterpreterBreadthSuite,
        TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF,
    };

    #[test]
    fn article_interpreter_breadth_suite_is_machine_legible() {
        let manifest = build_tassadar_article_interpreter_breadth_suite();
        assert_eq!(manifest.gate_issue_id, "TAS-179A");
        assert_eq!(
            manifest.envelope_manifest_ref,
            "fixtures/tassadar/sources/tassadar_article_interpreter_breadth_envelope_v1.json"
        );
        assert_eq!(manifest.required_family_ids.len(), 8);
        assert_eq!(manifest.family_rows.len(), 8);
    }

    #[test]
    fn article_interpreter_breadth_suite_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_interpreter_breadth_suite();
        let committed: TassadarArticleInterpreterBreadthSuite = serde_json::from_slice(
            &std::fs::read(tassadar_article_interpreter_breadth_suite_path())?,
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_interpreter_breadth_suite_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = std::env::temp_dir().join(format!(
            "psionic_tassadar_article_interpreter_breadth_suite_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory)?;
        let output_path = directory.join("tassadar_article_interpreter_breadth_suite_v1.json");
        let written = write_tassadar_article_interpreter_breadth_suite(&output_path)?;
        let persisted: TassadarArticleInterpreterBreadthSuite =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_interpreter_breadth_suite_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_interpreter_breadth_suite_v1.json")
        );
        assert_eq!(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_SUITE_REF,
            "fixtures/tassadar/sources/tassadar_article_interpreter_breadth_suite_v1.json"
        );
        std::fs::remove_file(&output_path)?;
        std::fs::remove_dir(&directory)?;
        Ok(())
    }
}
