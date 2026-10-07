use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable fixture ids for the bounded Rust-only article ABI lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleAbiFixtureId {
    ScalarAddOne,
    HeapSumI32,
    UnsupportedFloatParam,
    UnsupportedMultiResult,
}

impl TassadarArticleAbiFixtureId {
    /// Returns the stable fixture id string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScalarAddOne => "scalar_add_one",
            Self::HeapSumI32 => "heap_sum_i32",
            Self::UnsupportedFloatParam => "unsupported_float_param",
            Self::UnsupportedMultiResult => "unsupported_multi_result",
        }
    }
}

/// One parameter kind in the bounded Rust-only article ABI lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleAbiParamKind {
    /// One direct scalar `i32`.
    I32,
    /// One pointer into the invocation-owned `i32` heap buffer.
    PointerToI32,
    /// One element-count `i32` paired with a pointer parameter.
    LengthI32,
    /// Explicit unsupported floating-point boundary.
    F32,
    /// Explicit unsupported opaque host-handle boundary.
    HostHandle,
}

/// One result kind in the bounded Rust-only article ABI lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarArticleAbiResultKind {
    /// One direct scalar `i32`.
    I32,
    /// Explicit unsupported multi-result boundary.
    MultiI32Pair,
}

/// One heap-backed input layout in the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiHeapLayout {
    /// Parameter index carrying the pointer.
    pub pointer_param_index: u8,
    /// Parameter index carrying the element count.
    pub length_param_index: u8,
    /// Width in bytes for each heap element.
    pub element_width_bytes: u8,
    /// Whether heap loads should be interpreted as signed.
    pub signed: bool,
}

/// One compiler-facing canonical fixture for the bounded Rust-only article ABI lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarArticleAbiFixture {
    /// Stable fixture identifier.
    pub fixture_id: TassadarArticleAbiFixtureId,
    /// Stable source-canon case id anchoring the fixture.
    pub source_case_id: String,
    /// Stable repo-relative Rust source reference.
    pub source_ref: String,
    /// Exported entrypoint name.
    pub export_name: String,
    /// Stable workload-family id.
    pub workload_family_id: String,
    /// Declared parameter kinds in stable order.
    pub param_kinds: Vec<TassadarArticleAbiParamKind>,
    /// Declared result kinds in stable order.
    pub result_kinds: Vec<TassadarArticleAbiResultKind>,
    /// Optional heap-backed input layout.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heap_layout: Option<TassadarArticleAbiHeapLayout>,
    /// Plain-language claim boundary.
    pub claim_boundary: String,
    /// Stable digest over the fixture.
    pub fixture_digest: String,
}

impl TassadarArticleAbiFixture {
    fn new(
        fixture_id: TassadarArticleAbiFixtureId,
        source_case_id: impl Into<String>,
        source_ref: impl Into<String>,
        export_name: impl Into<String>,
        workload_family_id: impl Into<String>,
        param_kinds: Vec<TassadarArticleAbiParamKind>,
        result_kinds: Vec<TassadarArticleAbiResultKind>,
        heap_layout: Option<TassadarArticleAbiHeapLayout>,
        claim_boundary: impl Into<String>,
    ) -> Self {
        let mut fixture = Self {
            fixture_id,
            source_case_id: source_case_id.into(),
            source_ref: source_ref.into(),
            export_name: export_name.into(),
            workload_family_id: workload_family_id.into(),
            param_kinds,
            result_kinds,
            heap_layout,
            claim_boundary: claim_boundary.into(),
            fixture_digest: String::new(),
        };
        fixture.fixture_digest = stable_digest(b"psionic_tassadar_article_abi_fixture|", &fixture);
        fixture
    }

    /// Returns the canonical scalar-parameter fixture.
    #[must_use]
    pub fn scalar_add_one() -> Self {
        Self::new(
            TassadarArticleAbiFixtureId::ScalarAddOne,
            "param_abi_fixture",
            "fixtures/tassadar/sources/tassadar_param_abi_kernel.rs",
            "add_one",
            "rust.param_abi_kernel",
            vec![TassadarArticleAbiParamKind::I32],
            vec![TassadarArticleAbiResultKind::I32],
            None,
            "one direct scalar i32 parameter and one direct scalar i32 return are admitted under the bounded Rust-only article ABI lane",
        )
    }

    /// Returns the canonical pointer/length heap-backed fixture.
    #[must_use]
    pub fn heap_sum_i32() -> Self {
        Self::new(
            TassadarArticleAbiFixtureId::HeapSumI32,
            "heap_sum_article",
            "fixtures/tassadar/sources/tassadar_heap_sum_kernel.rs",
            "heap_sum_i32",
            "rust.heap_sum_article",
            vec![
                TassadarArticleAbiParamKind::PointerToI32,
                TassadarArticleAbiParamKind::LengthI32,
            ],
            vec![TassadarArticleAbiResultKind::I32],
            Some(TassadarArticleAbiHeapLayout {
                pointer_param_index: 0,
                length_param_index: 1,
                element_width_bytes: 4,
                signed: true,
            }),
            "one pointer-plus-length i32 heap input with one direct scalar i32 return is admitted under the bounded Rust-only article ABI lane",
        )
    }

    /// Returns one explicit unsupported floating-point parameter fixture.
    #[must_use]
    pub fn unsupported_float_param() -> Self {
        Self::new(
            TassadarArticleAbiFixtureId::UnsupportedFloatParam,
            "param_abi_fixture",
            "fixtures/tassadar/sources/tassadar_param_abi_kernel.rs",
            "add_one_f32",
            "rust.param_abi_kernel.unsupported_float",
            vec![TassadarArticleAbiParamKind::F32],
            vec![TassadarArticleAbiResultKind::I32],
            None,
            "floating-point parameter ABI shapes remain explicit refusals under the bounded Rust-only article ABI lane",
        )
    }

    /// Returns one explicit unsupported multi-result fixture.
    #[must_use]
    pub fn unsupported_multi_result() -> Self {
        Self::new(
            TassadarArticleAbiFixtureId::UnsupportedMultiResult,
            "param_abi_fixture",
            "fixtures/tassadar/sources/tassadar_param_abi_kernel.rs",
            "pair_sum_and_diff",
            "rust.param_abi_kernel.unsupported_multi_result",
            vec![TassadarArticleAbiParamKind::I32],
            vec![
                TassadarArticleAbiResultKind::I32,
                TassadarArticleAbiResultKind::MultiI32Pair,
            ],
            None,
            "multi-result ABI shapes remain explicit refusals under the bounded Rust-only article ABI lane",
        )
    }
}

/// Returns the canonical fixture suite for the bounded Rust-only article ABI lane.
#[must_use]
pub fn tassadar_article_abi_fixture_suite() -> Vec<TassadarArticleAbiFixture> {
    vec![
        TassadarArticleAbiFixture::scalar_add_one(),
        TassadarArticleAbiFixture::heap_sum_i32(),
        TassadarArticleAbiFixture::unsupported_float_param(),
        TassadarArticleAbiFixture::unsupported_multi_result(),
    ]
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
        TassadarArticleAbiFixtureId, TassadarArticleAbiParamKind, TassadarArticleAbiResultKind,
        tassadar_article_abi_fixture_suite,
    };

    #[test]
    fn article_abi_fixture_suite_is_machine_legible() {
        let fixtures = tassadar_article_abi_fixture_suite();

        assert_eq!(fixtures.len(), 4);
        assert!(fixtures.iter().any(|fixture| {
            fixture.fixture_id == TassadarArticleAbiFixtureId::ScalarAddOne
                && fixture.param_kinds == vec![TassadarArticleAbiParamKind::I32]
                && fixture.result_kinds == vec![TassadarArticleAbiResultKind::I32]
        }));
        assert!(fixtures.iter().any(|fixture| {
            fixture.fixture_id == TassadarArticleAbiFixtureId::HeapSumI32
                && fixture.heap_layout.is_some()
        }));
    }
}
