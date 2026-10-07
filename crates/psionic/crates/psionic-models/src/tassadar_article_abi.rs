use psionic_ir::TassadarArticleAbiFixture;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF: &str =
    "fixtures/tassadar/reports/tassadar_article_abi_closure_report.json";

/// Publication status for the bounded Rust-only article ABI lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleAbiPublicationStatus {
    Implemented,
}

/// Public model-facing publication for the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiPublication {
    pub schema_version: u16,
    pub publication_id: String,
    pub status: TassadarArticleAbiPublicationStatus,
    pub claim_class: String,
    pub family_id: String,
    pub report_ref: String,
    pub source_case_ids: Vec<String>,
    pub supported_entry_shapes: Vec<String>,
    pub refused_entry_shapes: Vec<String>,
    pub target_surfaces: Vec<String>,
    pub validation_refs: Vec<String>,
    pub claim_boundary: String,
    pub publication_digest: String,
}

impl TassadarArticleAbiPublication {
    fn new() -> Self {
        let fixtures = [
            TassadarArticleAbiFixture::scalar_add_one(),
            TassadarArticleAbiFixture::heap_sum_i32(),
        ];
        let mut publication = Self {
            schema_version: 1,
            publication_id: String::from("tassadar.article_abi.publication.v1"),
            status: TassadarArticleAbiPublicationStatus::Implemented,
            claim_class: String::from("compiled_bounded_exactness"),
            family_id: String::from("tassadar.rust_article_abi.v1"),
            report_ref: String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
            source_case_ids: fixtures
                .iter()
                .map(|fixture| fixture.source_case_id.clone())
                .collect(),
            supported_entry_shapes: vec![
                String::from("direct_scalar_i32_param_to_single_i32_return"),
                String::from("pointer_length_i32_heap_input_to_single_i32_return"),
            ],
            refused_entry_shapes: vec![
                String::from("floating_point_param_abi"),
                String::from("multi_result_return_abi"),
                String::from("host_handle_or_import_abi"),
            ],
            target_surfaces: vec![
                String::from("crates/psionic-ir"),
                String::from("crates/psionic-compiler"),
                String::from("crates/psionic-runtime"),
                String::from("crates/psionic-models"),
                String::from("crates/psionic-eval"),
            ],
            validation_refs: vec![
                String::from("fixtures/tassadar/reports/tassadar_rust_source_canon_report.json"),
                String::from(TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF),
            ],
            claim_boundary: String::from(
                "this publication closes direct scalar i32 and pointer-plus-length i32 heap-input entrypoints for the committed Rust-only article fixtures. It does not claim floating-point ABI closure, multi-result returns, host-handle imports, or arbitrary frontend and Wasm ABI closure",
            ),
            publication_digest: String::new(),
        };
        publication.publication_digest =
            stable_digest(b"psionic_tassadar_article_abi_publication|", &publication);
        publication
    }
}

/// Returns the canonical public publication for the bounded Rust-only article ABI lane.
#[must_use]
pub fn tassadar_article_abi_publication() -> TassadarArticleAbiPublication {
    TassadarArticleAbiPublication::new()
}

fn stable_digest<T: Serialize>(prefix: &[u8], value: &T) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(serde_json::to_vec(value).unwrap_or_default());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{TassadarArticleAbiPublicationStatus, tassadar_article_abi_publication};

    #[test]
    fn article_abi_publication_is_machine_legible() {
        let publication = tassadar_article_abi_publication();

        assert_eq!(
            publication.status,
            TassadarArticleAbiPublicationStatus::Implemented
        );
        assert_eq!(publication.family_id, "tassadar.rust_article_abi.v1");
        assert_eq!(
            publication.report_ref,
            super::TASSADAR_ARTICLE_ABI_CLOSURE_REPORT_REF
        );
        assert_eq!(publication.supported_entry_shapes.len(), 2);
    }
}
