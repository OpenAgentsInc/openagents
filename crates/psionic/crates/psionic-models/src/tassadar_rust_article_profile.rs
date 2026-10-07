use psionic_runtime::TassadarWasmProfileId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_SCHEMA_VERSION: u16 = 1;
pub const TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_article_profile_completeness_report.json";
const TASSADAR_RUST_SOURCE_CANON_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_rust_source_canon_report.json";
const TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_compile_pipeline_matrix_report.json";
const TASSADAR_WASM_CONFORMANCE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_wasm_conformance_report.json";
const TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_wasm_instruction_coverage_report.json";
const TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_module_scale_workload_suite_report.json";
const TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_abi_closure_report.json";
const TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_compiled_article_closure_report.json";

/// Machine-legible publication status for the Rust-to-Wasm article profile family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRustArticleProfilePublicationStatus {
    /// Landed as a repo-backed public substrate surface.
    Implemented,
}

/// One category of claim in the Rust-to-Wasm article profile matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRustArticleProfileCategory {
    /// Which module sections or module-shape families are in or out.
    ModuleSectionFamily,
    /// Which control-flow families are in or out.
    ControlFlowFamily,
    /// Which globals, tables, and indirect-call shapes are in or out.
    TableGlobalIndirectCallShape,
    /// Which numeric families are in or out.
    NumericFamily,
    /// Which ABI shapes are in or out.
    AbiShape,
}

/// Publication posture for one article-profile matrix row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarRustArticleProfileRowStatus {
    /// This family is currently part of the honest article profile boundary.
    Supported,
    /// This family remains outside the honest article profile boundary.
    Refused,
}

/// One machine-legible row in the Rust-to-Wasm article profile completeness matrix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustArticleProfileCompletenessRow {
    /// Stable row identifier.
    pub row_id: String,
    /// Category of the row.
    pub category: TassadarRustArticleProfileCategory,
    /// Stable feature identifier under the category.
    pub feature_id: String,
    /// Whether the family is supported or refused.
    pub status: TassadarRustArticleProfileRowStatus,
    /// Runtime profile ids that own this row.
    pub runtime_profile_ids: Vec<String>,
    /// Canonical Rust-source canon case ids anchoring this row when applicable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_case_ids: Vec<String>,
    /// Report refs or evidence anchors supporting the row.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supporting_report_refs: Vec<String>,
    /// Plain-language detail for the row.
    pub detail: String,
}

/// Public publication for the Rust-to-Wasm article profile completeness family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarRustArticleProfileCompletenessPublication {
    /// Stable schema version.
    pub schema_version: u16,
    /// Stable publication identifier.
    pub publication_id: String,
    /// Repo status vocabulary value for the family.
    pub status: TassadarRustArticleProfilePublicationStatus,
    /// Explicit claim class for the family.
    pub claim_class: String,
    /// Stable family identifier.
    pub family_id: String,
    /// Stable report ref for the committed JSON artifact.
    pub report_ref: String,
    /// Runtime profile ids that make up the current article family.
    pub runtime_profile_ids: Vec<String>,
    /// Stable source-canon case ids that anchor the family today.
    pub source_case_ids: Vec<String>,
    /// Supporting report refs that keep the family challengeable.
    pub validation_refs: Vec<String>,
    /// Explicit supported and refused rows in stable order.
    pub rows: Vec<TassadarRustArticleProfileCompletenessRow>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the publication.
    pub publication_digest: String,
}

impl TassadarRustArticleProfileCompletenessPublication {
    fn new() -> Self {
        let runtime_profile_ids = vec![
            String::from(TassadarWasmProfileId::ArticleI32ComputeV1.as_str()),
            String::from(TassadarWasmProfileId::Hungarian10x10MatchingV1.as_str()),
            String::from(TassadarWasmProfileId::Sudoku9x9SearchV1.as_str()),
        ];
        let source_case_ids = vec![
            String::from("multi_export_exact"),
            String::from("memory_lookup_exact"),
            String::from("param_abi_fixture"),
            String::from("micro_wasm_article"),
            String::from("heap_sum_article"),
            String::from("long_loop_article"),
            String::from("hungarian_10x10_article"),
            String::from("sudoku_9x9_article"),
        ];
        let validation_refs = vec![
            String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
            String::from(TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF),
            String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF),
            String::from(TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF),
            String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF),
            String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
            String::from(TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF),
        ];
        let rows = vec![
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from("module_sections.core_sections_with_memory_globals_data"),
                category: TassadarRustArticleProfileCategory::ModuleSectionFamily,
                feature_id: String::from("core_sections_with_memory_globals_exports_data"),
                status: TassadarRustArticleProfileRowStatus::Supported,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: source_case_ids.clone(),
                supporting_report_refs: vec![
                    String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
                    String::from(TASSADAR_COMPILE_PIPELINE_MATRIX_REPORT_REF),
                ],
                detail: String::from(
                    "the committed Rust source canon compiles into ordinary core Wasm modules with memory, globals, exports, data, and code sections; this is the real Rust-only frontend anchor for the article family",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from("control_flow.structured_loops_and_bounded_backtracking"),
                category: TassadarRustArticleProfileCategory::ControlFlowFamily,
                feature_id: String::from("structured_loops_and_bounded_backtracking"),
                status: TassadarRustArticleProfileRowStatus::Supported,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: vec![
                    String::from("long_loop_article"),
                    String::from("hungarian_10x10_article"),
                    String::from("sudoku_9x9_article"),
                ],
                supporting_report_refs: vec![
                    String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
                    String::from(TASSADAR_COMPILED_ARTICLE_CLOSURE_REPORT_REF),
                ],
                detail: String::from(
                    "the current article family honestly includes straight-line kernels, structured loops, and bounded search/backtracking programs, not only hand-held microprograms",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from(
                    "tables_globals_indirect_calls.single_funcref_table_mutable_i32_globals",
                ),
                category: TassadarRustArticleProfileCategory::TableGlobalIndirectCallShape,
                feature_id: String::from(
                    "single_funcref_table_mutable_i32_globals_zero_param_indirect_call",
                ),
                status: TassadarRustArticleProfileRowStatus::Supported,
                runtime_profile_ids: vec![String::from(
                    TassadarWasmProfileId::ArticleI32ComputeV1.as_str(),
                )],
                source_case_ids: Vec::new(),
                supporting_report_refs: vec![String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF)],
                detail: String::from(
                    "the bounded module lane already admits mutable i32 globals, one funcref table, and zero-parameter indirect calls, and that support is part of the honest article family rather than hidden substrate trivia",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from("numeric.i32_integer_family"),
                category: TassadarRustArticleProfileCategory::NumericFamily,
                feature_id: String::from("i32_integer_family"),
                status: TassadarRustArticleProfileRowStatus::Supported,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: source_case_ids.clone(),
                supporting_report_refs: vec![
                    String::from(TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF),
                    String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
                ],
                detail: String::from(
                    "the reproduced Rust article family is explicitly i32-first today; integer arithmetic, comparison, locals, and memory traffic are real and benchmark-backed",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from("abi.zero_param_exports_and_pointer_length_memory_inputs"),
                category: TassadarRustArticleProfileCategory::AbiShape,
                feature_id: String::from("zero_param_exports_and_pointer_length_memory_inputs"),
                status: TassadarRustArticleProfileRowStatus::Supported,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: vec![
                    String::from("multi_export_exact"),
                    String::from("memory_lookup_exact"),
                    String::from("micro_wasm_article"),
                    String::from("heap_sum_article"),
                    String::from("long_loop_article"),
                    String::from("hungarian_10x10_article"),
                    String::from("sudoku_9x9_article"),
                ],
                supporting_report_refs: vec![
                    String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
                    String::from(TASSADAR_MODULE_SCALE_WORKLOAD_SUITE_REPORT_REF),
                    String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
                ],
                detail: String::from(
                    "the current Rust article family honestly supports zero-parameter exported entrypoints plus pointer-length memory-style inputs, including the bounded direct pointer-plus-length heap-input closure proven by the Rust-only article ABI lane",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from("abi.direct_scalar_i32_and_pointer_length_single_i32_return"),
                category: TassadarRustArticleProfileCategory::AbiShape,
                feature_id: String::from("direct_scalar_i32_and_pointer_length_single_i32_return"),
                status: TassadarRustArticleProfileRowStatus::Supported,
                runtime_profile_ids: vec![String::from(
                    TassadarWasmProfileId::ArticleI32ComputeV1.as_str(),
                )],
                source_case_ids: vec![
                    String::from("param_abi_fixture"),
                    String::from("heap_sum_article"),
                ],
                supporting_report_refs: vec![
                    String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
                    String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
                ],
                detail: String::from(
                    "the bounded Rust-only article ABI lane now closes direct scalar i32 entrypoints plus pointer-length i32 heap-input entrypoints with one direct scalar i32 return on the committed canonical fixtures; broader ABI families remain explicit refusals",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from("module_sections.arbitrary_host_imports_and_component_shapes"),
                category: TassadarRustArticleProfileCategory::ModuleSectionFamily,
                feature_id: String::from("arbitrary_host_imports_and_component_shapes"),
                status: TassadarRustArticleProfileRowStatus::Refused,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: Vec::new(),
                supporting_report_refs: vec![String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF)],
                detail: String::from(
                    "arbitrary host-import sections and broader component-style module shapes remain out of scope; only deterministic zero-side-effect stubs are admitted in the bounded module lane",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from(
                    "control_flow.exception_handling_and_general_callstack_control",
                ),
                category: TassadarRustArticleProfileCategory::ControlFlowFamily,
                feature_id: String::from("exception_handling_and_general_callstack_control"),
                status: TassadarRustArticleProfileRowStatus::Refused,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: Vec::new(),
                supporting_report_refs: vec![String::from(
                    TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF,
                )],
                detail: String::from(
                    "the reproduced article family does not yet claim exception handling, arbitrary host-driven control transfer, or a general unconstrained call-stack model",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from(
                    "tables_globals_indirect_calls.multi_table_and_parametric_indirect",
                ),
                category: TassadarRustArticleProfileCategory::TableGlobalIndirectCallShape,
                feature_id: String::from(
                    "multi_table_multi_memory_typed_reference_parametric_indirect_call",
                ),
                status: TassadarRustArticleProfileRowStatus::Refused,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: Vec::new(),
                supporting_report_refs: vec![String::from(TASSADAR_WASM_CONFORMANCE_REPORT_REF)],
                detail: String::from(
                    "the honest article family stops at one funcref table, i32 globals, and zero-parameter indirect calls; broader typed-reference, multi-table, and parametric indirect-call families remain outside the claim",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from("numeric.i64_f32_f64_numeric_families"),
                category: TassadarRustArticleProfileCategory::NumericFamily,
                feature_id: String::from("i64_f32_f64_numeric_families"),
                status: TassadarRustArticleProfileRowStatus::Refused,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: Vec::new(),
                supporting_report_refs: vec![String::from(
                    TASSADAR_WASM_INSTRUCTION_COVERAGE_REPORT_REF,
                )],
                detail: String::from(
                    "the reproduced Rust article family remains explicitly i32-only; i64 and floating-point closure are not yet part of the public boundary",
                ),
            },
            TassadarRustArticleProfileCompletenessRow {
                row_id: String::from("abi.direct_parameter_exports_and_general_return_abi"),
                category: TassadarRustArticleProfileCategory::AbiShape,
                feature_id: String::from("direct_parameter_exports_and_general_return_abi"),
                status: TassadarRustArticleProfileRowStatus::Refused,
                runtime_profile_ids: runtime_profile_ids.clone(),
                source_case_ids: Vec::new(),
                supporting_report_refs: vec![
                    String::from(TASSADAR_RUST_SOURCE_CANON_REPORT_REF),
                    String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
                ],
                detail: String::from(
                    "the reproduced article family now includes bounded direct scalar i32 and pointer-length heap-input entrypoints, but broader parameter families, floating-point ABI, multi-result returns, and general host ABI closure remain explicit refusals",
                ),
            },
        ];

        let mut publication = Self {
            schema_version: TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_SCHEMA_VERSION,
            publication_id: String::from(
                "tassadar.rust_article_profile_completeness.publication.v1",
            ),
            status: TassadarRustArticleProfilePublicationStatus::Implemented,
            claim_class: String::from("execution_truth_profile_boundary"),
            family_id: String::from("tassadar.wasm.rust_article_family.v1"),
            report_ref: String::from(TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF),
            runtime_profile_ids,
            source_case_ids,
            validation_refs,
            rows,
            claim_boundary: String::from(
                "this publication freezes the current Rust-to-Wasm article profile family for Tassadar: it is strong enough to replace vague article rhetoric with supported and refused families across module shape, control flow, tables/globals/indirect calls, numeric families, and ABI shape. It remains a bounded i32-first family with explicit direct scalar i32 and pointer-length heap-input closure only, and it does not imply arbitrary Wasm, arbitrary Rust frontend closure, broad host-import closure, or general parameter ABI closure.",
            ),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_rust_article_profile_completeness_publication|",
            &publication,
        );
        publication
    }

    /// Returns whether the publication is explicit enough to surface publicly.
    pub fn validate(&self) -> Result<(), TassadarRustArticleProfilePublicationError> {
        if self.report_ref.trim().is_empty() {
            return Err(TassadarRustArticleProfilePublicationError::MissingReportRef);
        }
        if self.family_id.trim().is_empty() {
            return Err(TassadarRustArticleProfilePublicationError::MissingFamilyId);
        }
        if self.runtime_profile_ids.is_empty() {
            return Err(TassadarRustArticleProfilePublicationError::MissingRuntimeProfiles);
        }
        if self.rows.is_empty() {
            return Err(TassadarRustArticleProfilePublicationError::MissingRows);
        }
        if self.rows.iter().any(|row| row.row_id.trim().is_empty()) {
            return Err(TassadarRustArticleProfilePublicationError::InvalidRowId);
        }
        if !self
            .rows
            .iter()
            .any(|row| row.status == TassadarRustArticleProfileRowStatus::Supported)
        {
            return Err(TassadarRustArticleProfilePublicationError::MissingSupportedRows);
        }
        if !self
            .rows
            .iter()
            .any(|row| row.status == TassadarRustArticleProfileRowStatus::Refused)
        {
            return Err(TassadarRustArticleProfilePublicationError::MissingRefusedRows);
        }
        Ok(())
    }
}

/// Publication-validation failure for the Rust-to-Wasm article profile family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TassadarRustArticleProfilePublicationError {
    /// The publication omitted the report ref.
    MissingReportRef,
    /// The publication omitted the family id.
    MissingFamilyId,
    /// The publication omitted runtime profiles.
    MissingRuntimeProfiles,
    /// The publication omitted rows.
    MissingRows,
    /// One row id was empty.
    InvalidRowId,
    /// The publication omitted supported rows.
    MissingSupportedRows,
    /// The publication omitted refused rows.
    MissingRefusedRows,
}

/// Returns the canonical Rust-to-Wasm article profile completeness publication.
#[must_use]
pub fn tassadar_rust_article_profile_completeness_publication()
-> TassadarRustArticleProfileCompletenessPublication {
    TassadarRustArticleProfileCompletenessPublication::new()
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
        TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF,
        TassadarRustArticleProfilePublicationStatus, TassadarRustArticleProfileRowStatus,
        tassadar_rust_article_profile_completeness_publication,
    };

    #[test]
    fn rust_article_profile_publication_is_machine_legible() {
        let publication = tassadar_rust_article_profile_completeness_publication();

        assert_eq!(
            publication.status,
            TassadarRustArticleProfilePublicationStatus::Implemented
        );
        assert_eq!(
            publication.report_ref,
            TASSADAR_RUST_ARTICLE_PROFILE_COMPLETENESS_REPORT_REF
        );
        assert_eq!(publication.runtime_profile_ids.len(), 3);
        assert!(
            publication
                .rows
                .iter()
                .any(|row| row.status == TassadarRustArticleProfileRowStatus::Supported)
        );
        assert!(
            publication
                .rows
                .iter()
                .any(|row| row.status == TassadarRustArticleProfileRowStatus::Refused)
        );
        assert!(publication.validate().is_ok());
        assert!(!publication.publication_digest.is_empty());
    }
}
