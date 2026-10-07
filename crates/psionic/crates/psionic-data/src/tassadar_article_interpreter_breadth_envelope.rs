use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF: &str =
    "fixtures/tassadar/sources/tassadar_article_interpreter_breadth_envelope_v1.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleInterpreterFamilyId {
    FrozenCoreWasmWindow,
    ArticleNamedI32Profiles,
    SearchProcessFamily,
    LongHorizonControlFamily,
    ModuleScaleWasmLoopFamily,
    LinkedProgramBundleFamily,
    ImportMediatedProcessFamily,
    DynamicMemoryResumeFamily,
    Memory64Family,
    MultiMemoryFamily,
    ComponentLinkingFamily,
    ExceptionProfileFamily,
    FloatSemanticsFamily,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleInterpreterBreadthPosture {
    CurrentFloor,
    DeclaredRequiredFamily,
    ExplicitlyOutsideEnvelope,
    ResearchOnlyOutsideEnvelope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthEnvelopeRow {
    pub family_id: TassadarArticleInterpreterFamilyId,
    pub posture: TassadarArticleInterpreterBreadthPosture,
    pub authority_refs: Vec<String>,
    pub owner_surface_refs: Vec<String>,
    pub detail: String,
}

impl TassadarArticleInterpreterBreadthEnvelopeRow {
    fn validate(&self) -> Result<(), TassadarArticleInterpreterBreadthEnvelopeError> {
        if self.authority_refs.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingAuthorityRefs {
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
                TassadarArticleInterpreterBreadthEnvelopeError::InvalidAuthorityRef {
                    family_id: self.family_id,
                },
            );
        }
        if self.owner_surface_refs.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingOwnerSurfaceRefs {
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
                TassadarArticleInterpreterBreadthEnvelopeError::InvalidOwnerSurfaceRef {
                    family_id: self.family_id,
                },
            );
        }
        if self.detail.trim().is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingDetail {
                    family_id: self.family_id,
                },
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleInterpreterBreadthEnvelope {
    pub schema_version: u16,
    pub manifest_id: String,
    pub manifest_ref: String,
    pub route_anchor: String,
    pub suite_follow_on_issue_id: String,
    pub current_floor_family_ids: Vec<TassadarArticleInterpreterFamilyId>,
    pub declared_required_family_ids: Vec<TassadarArticleInterpreterFamilyId>,
    pub explicit_out_of_envelope_family_ids: Vec<TassadarArticleInterpreterFamilyId>,
    pub research_only_family_ids: Vec<TassadarArticleInterpreterFamilyId>,
    pub family_rows: Vec<TassadarArticleInterpreterBreadthEnvelopeRow>,
    pub current_truth_boundary: String,
    pub non_implications: Vec<String>,
    pub claim_boundary: String,
    pub manifest_digest: String,
}

impl TassadarArticleInterpreterBreadthEnvelope {
    fn new() -> Self {
        let mut manifest = Self {
            schema_version: 1,
            manifest_id: String::from("tassadar.article_interpreter_breadth_envelope.v1"),
            manifest_ref: String::from(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF),
            route_anchor: String::from("psionic_transformer.article_route"),
            suite_follow_on_issue_id: String::from("TAS-179A"),
            current_floor_family_ids: vec![
                TassadarArticleInterpreterFamilyId::FrozenCoreWasmWindow,
                TassadarArticleInterpreterFamilyId::ArticleNamedI32Profiles,
            ],
            declared_required_family_ids: vec![
                TassadarArticleInterpreterFamilyId::SearchProcessFamily,
                TassadarArticleInterpreterFamilyId::LongHorizonControlFamily,
                TassadarArticleInterpreterFamilyId::ModuleScaleWasmLoopFamily,
            ],
            explicit_out_of_envelope_family_ids: vec![
                TassadarArticleInterpreterFamilyId::ImportMediatedProcessFamily,
                TassadarArticleInterpreterFamilyId::DynamicMemoryResumeFamily,
                TassadarArticleInterpreterFamilyId::Memory64Family,
                TassadarArticleInterpreterFamilyId::MultiMemoryFamily,
                TassadarArticleInterpreterFamilyId::ComponentLinkingFamily,
                TassadarArticleInterpreterFamilyId::ExceptionProfileFamily,
                TassadarArticleInterpreterFamilyId::FloatSemanticsFamily,
            ],
            research_only_family_ids: vec![
                TassadarArticleInterpreterFamilyId::LinkedProgramBundleFamily,
            ],
            family_rows: family_rows(),
            current_truth_boundary: String::from(
                "the public repo now declares one article interpreter breadth envelope anchored on the frozen int-first core-Wasm window plus the current named i32-oriented article profiles, while later breadth closure is explicitly limited to the bounded search-process, long-horizon control, and module-scale Wasm-loop families. Linked-program bundles stay research-only, and import-mediated processes, dynamic-memory resume, memory64, multi-memory, component-linking, exception profiles, and broader float-semantics lanes remain outside the declared article envelope.",
            ),
            non_implications: vec![
                String::from("not arbitrary programs inside transformer weights"),
                String::from("not full core-Wasm public closure"),
                String::from("not arbitrary host-import or OS-mediated process closure"),
                String::from("not memory64, multi-memory, component-linking, or exception-profile closure"),
                String::from("not broad floating-point or mixed-numeric closure"),
                String::from("not final article-equivalence green status"),
            ],
            claim_boundary: String::from(
                "this manifest declares only the interpreter breadth envelope that later article-equivalence breadth claims are allowed to rely on. It does not by itself prove the declared family suite, arbitrary-program closure, or final article-equivalence green status.",
            ),
            manifest_digest: String::new(),
        };
        manifest
            .validate()
            .expect("article interpreter breadth envelope should validate");
        manifest.manifest_digest = stable_digest(
            b"psionic_tassadar_article_interpreter_breadth_envelope|",
            &manifest,
        );
        manifest
    }

    pub fn validate(&self) -> Result<(), TassadarArticleInterpreterBreadthEnvelopeError> {
        if self.manifest_id.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthEnvelopeError::MissingManifestId);
        }
        if self.manifest_ref.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthEnvelopeError::MissingManifestRef);
        }
        if self.route_anchor.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthEnvelopeError::MissingRouteAnchor);
        }
        if self.suite_follow_on_issue_id.trim().is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingSuiteFollowOnIssueId,
            );
        }
        if self.current_floor_family_ids.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingCurrentFloorFamilies,
            );
        }
        if self.declared_required_family_ids.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingDeclaredRequiredFamilies,
            );
        }
        if self.explicit_out_of_envelope_family_ids.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingExplicitOutOfEnvelopeFamilies,
            );
        }
        if self.research_only_family_ids.is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingResearchOnlyFamilies,
            );
        }
        if self.family_rows.is_empty() {
            return Err(TassadarArticleInterpreterBreadthEnvelopeError::MissingFamilyRows);
        }
        if self.current_truth_boundary.trim().is_empty() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::MissingCurrentTruthBoundary,
            );
        }
        if self.non_implications.is_empty() {
            return Err(TassadarArticleInterpreterBreadthEnvelopeError::MissingNonImplications);
        }
        if self
            .non_implications
            .iter()
            .any(|non_implication| non_implication.trim().is_empty())
        {
            return Err(TassadarArticleInterpreterBreadthEnvelopeError::InvalidNonImplication);
        }
        if self.claim_boundary.trim().is_empty() {
            return Err(TassadarArticleInterpreterBreadthEnvelopeError::MissingClaimBoundary);
        }

        let all_group_ids = self
            .current_floor_family_ids
            .iter()
            .chain(self.declared_required_family_ids.iter())
            .chain(self.explicit_out_of_envelope_family_ids.iter())
            .chain(self.research_only_family_ids.iter())
            .copied()
            .collect::<Vec<_>>();
        if all_group_ids.len() != all_group_ids.iter().copied().collect::<BTreeSet<_>>().len() {
            return Err(TassadarArticleInterpreterBreadthEnvelopeError::DuplicateFamilyAssignment);
        }

        let mut seen_rows = BTreeSet::new();
        for row in &self.family_rows {
            row.validate()?;
            if !seen_rows.insert(row.family_id) {
                return Err(
                    TassadarArticleInterpreterBreadthEnvelopeError::DuplicateFamilyRow {
                        family_id: row.family_id,
                    },
                );
            }
            if row.posture != expected_posture(self, row.family_id)? {
                return Err(
                    TassadarArticleInterpreterBreadthEnvelopeError::FamilyPostureMismatch {
                        family_id: row.family_id,
                        expected: expected_posture(self, row.family_id)?,
                        actual: row.posture,
                    },
                );
            }
        }
        if seen_rows.len() != all_group_ids.len() {
            return Err(
                TassadarArticleInterpreterBreadthEnvelopeError::FamilyRowCountMismatch {
                    expected: all_group_ids.len(),
                    actual: seen_rows.len(),
                },
            );
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum TassadarArticleInterpreterBreadthEnvelopeError {
    #[error("article interpreter breadth envelope is missing `manifest_id`")]
    MissingManifestId,
    #[error("article interpreter breadth envelope is missing `manifest_ref`")]
    MissingManifestRef,
    #[error("article interpreter breadth envelope is missing `route_anchor`")]
    MissingRouteAnchor,
    #[error("article interpreter breadth envelope is missing `suite_follow_on_issue_id`")]
    MissingSuiteFollowOnIssueId,
    #[error("article interpreter breadth envelope is missing current-floor family ids")]
    MissingCurrentFloorFamilies,
    #[error("article interpreter breadth envelope is missing declared required family ids")]
    MissingDeclaredRequiredFamilies,
    #[error("article interpreter breadth envelope is missing explicit out-of-envelope family ids")]
    MissingExplicitOutOfEnvelopeFamilies,
    #[error("article interpreter breadth envelope is missing research-only family ids")]
    MissingResearchOnlyFamilies,
    #[error("article interpreter breadth envelope is missing `family_rows`")]
    MissingFamilyRows,
    #[error("article interpreter breadth envelope is missing `current_truth_boundary`")]
    MissingCurrentTruthBoundary,
    #[error("article interpreter breadth envelope is missing `non_implications`")]
    MissingNonImplications,
    #[error("article interpreter breadth envelope includes an empty non-implication entry")]
    InvalidNonImplication,
    #[error("article interpreter breadth envelope is missing `claim_boundary`")]
    MissingClaimBoundary,
    #[error("article interpreter breadth envelope assigns the same family to multiple groups")]
    DuplicateFamilyAssignment,
    #[error("article interpreter breadth envelope duplicated family row `{family_id:?}`")]
    DuplicateFamilyRow {
        family_id: TassadarArticleInterpreterFamilyId,
    },
    #[error("article interpreter breadth envelope is missing authority refs for `{family_id:?}`")]
    MissingAuthorityRefs {
        family_id: TassadarArticleInterpreterFamilyId,
    },
    #[error("article interpreter breadth envelope has an empty authority ref for `{family_id:?}`")]
    InvalidAuthorityRef {
        family_id: TassadarArticleInterpreterFamilyId,
    },
    #[error(
        "article interpreter breadth envelope is missing owner-surface refs for `{family_id:?}`"
    )]
    MissingOwnerSurfaceRefs {
        family_id: TassadarArticleInterpreterFamilyId,
    },
    #[error(
        "article interpreter breadth envelope has an empty owner-surface ref for `{family_id:?}`"
    )]
    InvalidOwnerSurfaceRef {
        family_id: TassadarArticleInterpreterFamilyId,
    },
    #[error("article interpreter breadth envelope is missing detail for `{family_id:?}`")]
    MissingDetail {
        family_id: TassadarArticleInterpreterFamilyId,
    },
    #[error(
        "article interpreter breadth envelope posture mismatch for `{family_id:?}`: expected `{expected:?}`, got `{actual:?}`"
    )]
    FamilyPostureMismatch {
        family_id: TassadarArticleInterpreterFamilyId,
        expected: TassadarArticleInterpreterBreadthPosture,
        actual: TassadarArticleInterpreterBreadthPosture,
    },
    #[error("article interpreter breadth envelope has no group assignment for `{family_id:?}`")]
    MissingFamilyAssignment {
        family_id: TassadarArticleInterpreterFamilyId,
    },
    #[error("article interpreter breadth envelope family-row count mismatch: expected {expected}, actual {actual}")]
    FamilyRowCountMismatch { expected: usize, actual: usize },
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

pub fn build_tassadar_article_interpreter_breadth_envelope(
) -> TassadarArticleInterpreterBreadthEnvelope {
    TassadarArticleInterpreterBreadthEnvelope::new()
}

pub fn tassadar_article_interpreter_breadth_envelope_path() -> PathBuf {
    repo_root().join(TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF)
}

pub fn write_tassadar_article_interpreter_breadth_envelope(
    output_path: impl AsRef<Path>,
) -> Result<TassadarArticleInterpreterBreadthEnvelope, TassadarArticleInterpreterBreadthEnvelopeError>
{
    let output_path = output_path.as_ref();
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            TassadarArticleInterpreterBreadthEnvelopeError::CreateDir {
                path: parent.display().to_string(),
                error,
            }
        })?;
    }
    let manifest = build_tassadar_article_interpreter_breadth_envelope();
    let json = serde_json::to_string_pretty(&manifest)?;
    fs::write(output_path, format!("{json}\n")).map_err(|error| {
        TassadarArticleInterpreterBreadthEnvelopeError::Write {
            path: output_path.display().to_string(),
            error,
        }
    })?;
    Ok(manifest)
}

fn expected_posture(
    manifest: &TassadarArticleInterpreterBreadthEnvelope,
    family_id: TassadarArticleInterpreterFamilyId,
) -> Result<TassadarArticleInterpreterBreadthPosture, TassadarArticleInterpreterBreadthEnvelopeError>
{
    if manifest.current_floor_family_ids.contains(&family_id) {
        Ok(TassadarArticleInterpreterBreadthPosture::CurrentFloor)
    } else if manifest.declared_required_family_ids.contains(&family_id) {
        Ok(TassadarArticleInterpreterBreadthPosture::DeclaredRequiredFamily)
    } else if manifest
        .explicit_out_of_envelope_family_ids
        .contains(&family_id)
    {
        Ok(TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope)
    } else if manifest.research_only_family_ids.contains(&family_id) {
        Ok(TassadarArticleInterpreterBreadthPosture::ResearchOnlyOutsideEnvelope)
    } else {
        Err(TassadarArticleInterpreterBreadthEnvelopeError::MissingFamilyAssignment { family_id })
    }
}

fn family_rows() -> Vec<TassadarArticleInterpreterBreadthEnvelopeRow> {
    vec![
        row(
            TassadarArticleInterpreterFamilyId::FrozenCoreWasmWindow,
            TassadarArticleInterpreterBreadthPosture::CurrentFloor,
            &[
                "fixtures/tassadar/reports/tassadar_frozen_core_wasm_window_report.json",
                "fixtures/tassadar/reports/tassadar_frozen_core_wasm_closure_gate_report.json",
            ],
            &[
                "crates/psionic-ir/src/tassadar_frozen_core_wasm.rs",
                "crates/psionic-compiler/src/tassadar_wasm_module.rs",
                "crates/psionic-runtime/src/tassadar_frozen_core_wasm.rs",
            ],
            "the declared frozen int-first core-Wasm window is the widest public interpreter floor the article-closure wave may currently cite, even though full public core-Wasm publication remains separately suppressed",
        ),
        row(
            TassadarArticleInterpreterFamilyId::ArticleNamedI32Profiles,
            TassadarArticleInterpreterBreadthPosture::CurrentFloor,
            &[
                "fixtures/tassadar/reports/tassadar_rust_article_profile_completeness_report.json",
                "fixtures/tassadar/reports/tassadar_article_abi_closure_report.json",
                "fixtures/tassadar/reports/tassadar_article_frontend_compiler_envelope_report.json",
                "fixtures/tassadar/reports/tassadar_article_frontend_corpus_compile_matrix_report.json",
                "fixtures/tassadar/reports/tassadar_article_demo_frontend_parity_report.json",
            ],
            &[
                "crates/psionic-transformer/src/lib.rs",
                "crates/psionic-compiler/src/tassadar_article_frontend_compiler_envelope.rs",
                "crates/psionic-runtime/src/tassadar_article_abi.rs",
            ],
            "current public article execution claims stay further bounded to the named i32-oriented article profiles and their committed Rust-source ingress instead of generic arbitrary-program closure",
        ),
        row(
            TassadarArticleInterpreterFamilyId::SearchProcessFamily,
            TassadarArticleInterpreterBreadthPosture::DeclaredRequiredFamily,
            &[
                "fixtures/tassadar/reports/tassadar_search_native_executor_report.json",
                "fixtures/tassadar/reports/tassadar_hungarian_10x10_article_reproducer_report.json",
                "fixtures/tassadar/reports/tassadar_sudoku_9x9_article_reproducer_report.json",
            ],
            &[
                "crates/psionic-runtime/src/tassadar_search_native_executor.rs",
                "crates/psionic-runtime/src/tassadar.rs",
                "crates/psionic-compiler/src/tassadar_wasm_module.rs",
            ],
            "later article breadth closure must explicitly cover bounded search-process programs rather than inferring arbitrary-program breadth from one or two demo rows",
        ),
        row(
            TassadarArticleInterpreterFamilyId::LongHorizonControlFamily,
            TassadarArticleInterpreterBreadthPosture::DeclaredRequiredFamily,
            &[
                "fixtures/tassadar/reports/tassadar_article_runtime_closeout_report.json",
            ],
            &[
                "crates/psionic-runtime/src/tassadar_article_runtime_closeout.rs",
                "crates/psionic-runtime/src/tassadar.rs",
            ],
            "later article breadth closure must explicitly cover long-horizon control programs inside the declared article route instead of treating the existing runtime closeout bundle as generic arbitrary-program evidence",
        ),
        row(
            TassadarArticleInterpreterFamilyId::ModuleScaleWasmLoopFamily,
            TassadarArticleInterpreterBreadthPosture::DeclaredRequiredFamily,
            &[
                "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json",
            ],
            &[
                "crates/psionic-data/src/tassadar_kernel_module_scaling.rs",
                "crates/psionic-compiler/src/tassadar_wasm_module.rs",
                "crates/psionic-runtime/src/tassadar_module_execution.rs",
            ],
            "later article breadth closure must explicitly cover bounded module-scale loop programs instead of reading the frontend or demo tranches as sufficient by themselves",
        ),
        row(
            TassadarArticleInterpreterFamilyId::LinkedProgramBundleFamily,
            TassadarArticleInterpreterBreadthPosture::ResearchOnlyOutsideEnvelope,
            &[
                "fixtures/tassadar/reports/tassadar_module_link_runtime_report.json",
                "fixtures/tassadar/reports/tassadar_module_link_eval_report.json",
            ],
            &[
                "crates/psionic-compiler/src/tassadar_module_linker.rs",
                "crates/psionic-runtime/src/tassadar_linked_program_bundle.rs",
            ],
            "linked-program bundles stay visible as a research-only lane and are not part of the declared article interpreter breadth envelope",
        ),
        row(
            TassadarArticleInterpreterFamilyId::ImportMediatedProcessFamily,
            TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope,
            &[
                "fixtures/tassadar/reports/tassadar_effect_taxonomy_report.json",
                "fixtures/tassadar/reports/tassadar_simulator_effect_profile_report.json",
                "fixtures/tassadar/reports/tassadar_simulator_effect_sandbox_boundary_report.json",
            ],
            &[
                "crates/psionic-runtime/src/tassadar_effect_taxonomy.rs",
                "crates/psionic-runtime/src/tassadar_simulator_effect_profile.rs",
            ],
            "host-import and simulator-mediated process families remain explicit outside the declared article interpreter envelope",
        ),
        row(
            TassadarArticleInterpreterFamilyId::DynamicMemoryResumeFamily,
            TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope,
            &[
                "fixtures/tassadar/reports/tassadar_dynamic_memory_resume_report.json",
            ],
            &[
                "crates/psionic-runtime/src/tassadar_dynamic_memory_resume.rs",
                "crates/psionic-runtime/src/tassadar_execution_checkpoint.rs",
            ],
            "dynamic-memory pause-and-resume remains explicit outside the declared article envelope instead of silently inheriting from the bounded checkpoint substrate",
        ),
        row(
            TassadarArticleInterpreterFamilyId::Memory64Family,
            TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope,
            &[
                "fixtures/tassadar/reports/tassadar_memory64_profile_report.json",
            ],
            &[
                "crates/psionic-ir/src/tassadar_generalized_abi.rs",
                "crates/psionic-runtime/src/tassadar_memory64_profile.rs",
            ],
            "memory64 remains explicit outside the declared article interpreter envelope",
        ),
        row(
            TassadarArticleInterpreterFamilyId::MultiMemoryFamily,
            TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope,
            &[
                "fixtures/tassadar/reports/tassadar_multi_memory_profile_report.json",
            ],
            &[
                "crates/psionic-ir/src/tassadar_multi_memory_profile.rs",
                "crates/psionic-runtime/src/tassadar_multi_memory_profile.rs",
            ],
            "multi-memory remains explicit outside the declared article interpreter envelope",
        ),
        row(
            TassadarArticleInterpreterFamilyId::ComponentLinkingFamily,
            TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope,
            &[
                "fixtures/tassadar/reports/tassadar_component_linking_profile_report.json",
            ],
            &[
                "crates/psionic-ir/src/tassadar_component_linking_profile.rs",
                "crates/psionic-compiler/src/tassadar_component_linking_profile.rs",
                "crates/psionic-runtime/src/tassadar_component_linking_profile.rs",
            ],
            "component-linking remains explicit outside the declared article interpreter envelope",
        ),
        row(
            TassadarArticleInterpreterFamilyId::ExceptionProfileFamily,
            TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope,
            &[
                "fixtures/tassadar/reports/tassadar_exception_profile_report.json",
                "fixtures/tassadar/reports/tassadar_exception_profile_runtime_report.json",
            ],
            &[
                "crates/psionic-ir/src/tassadar_exception_profile.rs",
                "crates/psionic-compiler/src/tassadar_exception_profile.rs",
                "crates/psionic-runtime/src/tassadar_exception_profile.rs",
            ],
            "exception-profile execution remains explicit outside the declared article interpreter envelope",
        ),
        row(
            TassadarArticleInterpreterFamilyId::FloatSemanticsFamily,
            TassadarArticleInterpreterBreadthPosture::ExplicitlyOutsideEnvelope,
            &[
                "fixtures/tassadar/reports/tassadar_float_semantics_comparison_matrix_report.json",
            ],
            &[
                "crates/psionic-compiler/src/tassadar_float_semantics.rs",
                "crates/psionic-runtime/src/tassadar_float_semantics.rs",
            ],
            "broader floating-point and mixed-numeric semantics remain explicit outside the declared article interpreter envelope",
        ),
    ]
}

fn row(
    family_id: TassadarArticleInterpreterFamilyId,
    posture: TassadarArticleInterpreterBreadthPosture,
    authority_refs: &[&str],
    owner_surface_refs: &[&str],
    detail: &str,
) -> TassadarArticleInterpreterBreadthEnvelopeRow {
    TassadarArticleInterpreterBreadthEnvelopeRow {
        family_id,
        posture,
        authority_refs: authority_refs
            .iter()
            .map(|value| String::from(*value))
            .collect(),
        owner_surface_refs: owner_surface_refs
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
        build_tassadar_article_interpreter_breadth_envelope,
        tassadar_article_interpreter_breadth_envelope_path,
        write_tassadar_article_interpreter_breadth_envelope,
        TassadarArticleInterpreterBreadthEnvelope,
        TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF,
    };

    #[test]
    fn article_interpreter_breadth_envelope_is_machine_legible() {
        let manifest = build_tassadar_article_interpreter_breadth_envelope();
        assert_eq!(manifest.route_anchor, "psionic_transformer.article_route");
        assert_eq!(manifest.suite_follow_on_issue_id, "TAS-179A");
        assert_eq!(manifest.current_floor_family_ids.len(), 2);
        assert_eq!(manifest.declared_required_family_ids.len(), 3);
        assert_eq!(manifest.explicit_out_of_envelope_family_ids.len(), 7);
        assert_eq!(manifest.research_only_family_ids.len(), 1);
        assert_eq!(manifest.family_rows.len(), 13);
    }

    #[test]
    fn article_interpreter_breadth_envelope_matches_committed_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let generated = build_tassadar_article_interpreter_breadth_envelope();
        let committed: TassadarArticleInterpreterBreadthEnvelope = serde_json::from_slice(
            &std::fs::read(tassadar_article_interpreter_breadth_envelope_path())?,
        )?;
        assert_eq!(generated, committed);
        Ok(())
    }

    #[test]
    fn write_article_interpreter_breadth_envelope_persists_current_truth(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let output_directory = std::env::temp_dir().join(format!(
            "psionic_tassadar_article_interpreter_breadth_envelope_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&output_directory)?;
        let output_path =
            output_directory.join("tassadar_article_interpreter_breadth_envelope_v1.json");
        let written = write_tassadar_article_interpreter_breadth_envelope(&output_path)?;
        let persisted: TassadarArticleInterpreterBreadthEnvelope =
            serde_json::from_slice(&std::fs::read(&output_path)?)?;
        assert_eq!(written, persisted);
        assert_eq!(
            tassadar_article_interpreter_breadth_envelope_path()
                .file_name()
                .and_then(|value| value.to_str()),
            Some("tassadar_article_interpreter_breadth_envelope_v1.json")
        );
        assert_eq!(
            TASSADAR_ARTICLE_INTERPRETER_BREADTH_ENVELOPE_REF,
            "fixtures/tassadar/sources/tassadar_article_interpreter_breadth_envelope_v1.json"
        );
        std::fs::remove_file(&output_path)?;
        std::fs::remove_dir(&output_directory)?;
        Ok(())
    }
}
