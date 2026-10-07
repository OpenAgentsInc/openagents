use psionic_ir::TassadarGeneralizedAbiFixture;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_generalized_abi_family_report.json";

/// Publication status for the generalized ABI family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralizedAbiPublicationStatus {
    Implemented,
}

/// Public model-facing publication for the generalized ABI family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralizedAbiPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarGeneralizedAbiPublicationStatus,
    pub claim_class: String,
    pub family_id: String,
    pub report_ref: String,
    pub source_case_ids: Vec<String>,
    pub supported_program_shape_ids: Vec<String>,
    pub supported_runtime_support_ids: Vec<String>,
    pub ownership_rules: Vec<String>,
    pub refused_shape_ids: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub claim_boundary: String,
    pub publication_digest: String,
}

impl TassadarGeneralizedAbiPublication {
    fn new() -> Self {
        let fixtures = [
            TassadarGeneralizedAbiFixture::pair_add_i32(),
            TassadarGeneralizedAbiFixture::pair_add_i64(),
            TassadarGeneralizedAbiFixture::dual_heap_dot_i32(),
            TassadarGeneralizedAbiFixture::sum_and_max_status_output(),
            TassadarGeneralizedAbiFixture::pair_sum_and_diff_i32(),
            TassadarGeneralizedAbiFixture::sum_and_max_i64_status_output(),
            TassadarGeneralizedAbiFixture::multi_export_pair_sum(),
            TassadarGeneralizedAbiFixture::multi_export_local_double(),
        ];
        let mut source_case_ids = fixtures
            .iter()
            .map(|fixture| fixture.source_case_id.clone())
            .collect::<Vec<_>>();
        source_case_ids.sort();
        source_case_ids.dedup();
        let mut supported_program_shape_ids = fixtures
            .iter()
            .map(|fixture| fixture.program_shape_id.clone())
            .collect::<Vec<_>>();
        supported_program_shape_ids.sort();
        supported_program_shape_ids.dedup();
        let mut supported_runtime_support_ids = fixtures
            .iter()
            .flat_map(|fixture| fixture.runtime_support_ids.clone())
            .collect::<Vec<_>>();
        supported_runtime_support_ids.sort();
        supported_runtime_support_ids.dedup();

        let mut publication = Self {
            schema_version: 1,
            publication_id: String::from("tassadar.generalized_abi.publication.v1"),
            status: TassadarGeneralizedAbiPublicationStatus::Implemented,
            claim_class: String::from("compiled_bounded_exactness"),
            family_id: String::from("tassadar.rust_generalized_abi.v1"),
            report_ref: String::from(TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF),
            source_case_ids,
            supported_program_shape_ids,
            supported_runtime_support_ids,
            ownership_rules: vec![
                String::from("caller_owns_all_input_buffers"),
                String::from("caller_owns_all_output_buffers"),
                String::from("output_buffers_must_not_alias_other_regions"),
                String::from("i64_buffer_regions_must_be_8_byte_aligned"),
                String::from("callee_allocated_returned_buffers_are_refused"),
                String::from("free_or_host_handle_callbacks_are_refused"),
            ],
            refused_shape_ids: vec![
                String::from("floating_point_param_abi"),
                String::from("mixed_multi_value_return_abi"),
                String::from("host_handle_or_callback_abi"),
                String::from("callee_allocated_returned_buffer"),
                String::from("dynamic_runtime_or_std_alloc"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-ir"),
                String::from("crates/psionic-compiler"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-environments"),
                String::from("crates/psionic-eval"),
                String::from("crates/psionic-serve"),
            ],
            validation_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_rust_source_canon_report.json"),
                String::from("fixtures/tassadar/reports/tassadar_article_abi_closure_report.json"),
                String::from(TASSADAR_GENERALIZED_ABI_FAMILY_REPORT_REF),
            ],
            claim_boundary: String::from(
                "this publication widens the bounded ABI story from the article-only direct ABI slice to one reusable generalized family over multi-param scalar entrypoints, exact i64 scalar entrypoints, homogeneous two-value i32 returns, multiple pointer-length inputs, caller-owned result-code-plus-output-buffer shapes, 8-byte caller-owned buffer layouts, and bounded multi-export program shapes. It does not claim floating-point closure, mixed-width multi-result returns, host-handle imports, callee-allocated returned buffers, arbitrary runtime support, or arbitrary Wasm ABI closure",
            ),
            publication_digest: String::new(),
        };
        publication.publication_digest = stable_digest(
            b"psionic_tassadar_generalized_abi_publication|",
            &publication,
        );
        publication
    }
}

/// Returns the canonical generalized ABI publication.
#[must_use]
pub fn tassadar_generalized_abi_publication() -> TassadarGeneralizedAbiPublication {
    TassadarGeneralizedAbiPublication::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TassadarGeneralizedAbiPublicationStatus, tassadar_generalized_abi_publication};

    #[test]
    fn generalized_abi_publication_is_machine_legible() {
        let publication = tassadar_generalized_abi_publication();

        assert_eq!(
            publication.status,
            TassadarGeneralizedAbiPublicationStatus::Implemented
        );
        assert_eq!(publication.family_id, "tassadar.rust_generalized_abi.v1");
        assert!(
            publication
                .supported_program_shape_ids
                .contains(&String::from("result_code_plus_output_buffer_i32"))
        );
        assert!(
            publication
                .supported_program_shape_ids
                .contains(&String::from("result_code_plus_output_buffer_i64"))
        );
    }
}
