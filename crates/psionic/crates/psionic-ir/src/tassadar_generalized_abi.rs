use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Stable fixture ids for the widened generalized ABI family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralizedAbiFixtureId {
    PairAddI32,
    PairAddI64,
    DualHeapDotI32,
    SumAndMaxStatusOutput,
    PairSumAndDiffI32,
    SumAndMaxI64StatusOutput,
    MultiExportPairSum,
    MultiExportLocalDouble,
    UnsupportedFloatParam,
    UnsupportedMultiResult,
    UnsupportedHostHandle,
    UnsupportedReturnedBuffer,
}

impl TassadarGeneralizedAbiFixtureId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PairAddI32 => "pair_add_i32",
            Self::PairAddI64 => "pair_add_i64",
            Self::DualHeapDotI32 => "dual_heap_dot_i32",
            Self::SumAndMaxStatusOutput => "sum_and_max_status_output",
            Self::PairSumAndDiffI32 => "pair_sum_and_diff_i32",
            Self::SumAndMaxI64StatusOutput => "sum_and_max_i64_status_output",
            Self::MultiExportPairSum => "multi_export_pair_sum",
            Self::MultiExportLocalDouble => "multi_export_local_double",
            Self::UnsupportedFloatParam => "unsupported_float_param",
            Self::UnsupportedMultiResult => "unsupported_multi_result",
            Self::UnsupportedHostHandle => "unsupported_host_handle",
            Self::UnsupportedReturnedBuffer => "unsupported_returned_buffer",
        }
    }
}

/// One parameter kind in the widened generalized ABI family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralizedAbiParamKind {
    I32,
    I64,
    PointerToI32,
    LengthI32,
    F32,
    HostHandle,
}

/// One result kind in the widened generalized ABI family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralizedAbiResultKind {
    I32,
    I64,
    BufferPointerAndLength,
}

/// One memory-region role in the widened generalized ABI family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TassadarGeneralizedAbiMemoryRegionRole {
    Input,
    Output,
}

/// One declared memory region for the widened generalized ABI family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralizedAbiMemoryRegion {
    pub region_id: String,
    pub role: TassadarGeneralizedAbiMemoryRegionRole,
    pub pointer_param_index: u8,
    pub length_param_index: u8,
    pub element_width_bytes: u8,
    pub signed: bool,
    pub minimum_length_elements: u8,
}

/// One canonical fixture for the widened generalized ABI family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TassadarGeneralizedAbiFixture {
    pub fixture_id: TassadarGeneralizedAbiFixtureId,
    pub source_case_id: String,
    pub source_ref: String,
    pub export_name: String,
    pub workload_family_id: String,
    pub program_shape_id: String,
    pub param_kinds: Vec<TassadarGeneralizedAbiParamKind>,
    pub result_kinds: Vec<TassadarGeneralizedAbiResultKind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory_regions: Vec<TassadarGeneralizedAbiMemoryRegion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_support_ids: Vec<String>,
    pub claim_boundary: String,
    pub fixture_digest: String,
}

impl TassadarGeneralizedAbiFixture {
    fn new(
        fixture_id: TassadarGeneralizedAbiFixtureId,
        source_case_id: impl Into<String>,
        source_ref: impl Into<String>,
        export_name: impl Into<String>,
        workload_family_id: impl Into<String>,
        program_shape_id: impl Into<String>,
        param_kinds: Vec<TassadarGeneralizedAbiParamKind>,
        result_kinds: Vec<TassadarGeneralizedAbiResultKind>,
        mut memory_regions: Vec<TassadarGeneralizedAbiMemoryRegion>,
        mut runtime_support_ids: Vec<String>,
        claim_boundary: impl Into<String>,
    ) -> Self {
        memory_regions.sort_by(|left, right| left.region_id.cmp(&right.region_id));
        runtime_support_ids.sort();
        runtime_support_ids.dedup();
        let mut fixture = Self {
            fixture_id,
            source_case_id: source_case_id.into(),
            source_ref: source_ref.into(),
            export_name: export_name.into(),
            workload_family_id: workload_family_id.into(),
            program_shape_id: program_shape_id.into(),
            param_kinds,
            result_kinds,
            memory_regions,
            runtime_support_ids,
            claim_boundary: claim_boundary.into(),
            fixture_digest: String::new(),
        };
        fixture.fixture_digest =
            stable_digest(b"psionic_tassadar_generalized_abi_fixture|", &fixture);
        fixture
    }

    #[must_use]
    pub fn pair_add_i32() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::PairAddI32,
            "param_abi_fixture",
            "fixtures/tassadar/sources/tassadar_param_abi_kernel.rs",
            "pair_add",
            "rust.param_abi_kernel.generalized_pair_add",
            "multi_param_i32_to_single_i32_return",
            vec![
                TassadarGeneralizedAbiParamKind::I32,
                TassadarGeneralizedAbiParamKind::I32,
            ],
            vec![TassadarGeneralizedAbiResultKind::I32],
            Vec::new(),
            vec![
                String::from("panic_abort_loop_only"),
                String::from("direct_scalar_i32_entrypoints"),
            ],
            "two direct scalar i32 parameters with one direct scalar i32 return are admitted under the widened generalized ABI family",
        )
    }

    #[must_use]
    pub fn pair_add_i64() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::PairAddI64,
            "wider_numeric_pair_add",
            "fixtures/tassadar/sources/tassadar_wider_numeric_kernel.rs",
            "pair_add_i64",
            "rust.wider_numeric_kernel.pair_add_i64",
            "multi_param_i64_to_single_i64_return",
            vec![
                TassadarGeneralizedAbiParamKind::I64,
                TassadarGeneralizedAbiParamKind::I64,
            ],
            vec![TassadarGeneralizedAbiResultKind::I64],
            Vec::new(),
            vec![
                String::from("direct_scalar_i64_entrypoints"),
                String::from("panic_abort_loop_only"),
            ],
            "two direct scalar i64 parameters with one direct scalar i64 return are admitted under the widened numeric and data-layout ladder",
        )
    }

    #[must_use]
    pub fn dual_heap_dot_i32() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::DualHeapDotI32,
            "heap_sum_article",
            "fixtures/tassadar/sources/tassadar_heap_sum_kernel.rs",
            "dot_i32",
            "rust.heap_sum_article.generalized_dual_heap_dot",
            "multiple_pointer_length_inputs_to_single_i32_return",
            vec![
                TassadarGeneralizedAbiParamKind::PointerToI32,
                TassadarGeneralizedAbiParamKind::PointerToI32,
                TassadarGeneralizedAbiParamKind::LengthI32,
            ],
            vec![TassadarGeneralizedAbiResultKind::I32],
            vec![
                TassadarGeneralizedAbiMemoryRegion {
                    region_id: String::from("left_input"),
                    role: TassadarGeneralizedAbiMemoryRegionRole::Input,
                    pointer_param_index: 0,
                    length_param_index: 2,
                    element_width_bytes: 4,
                    signed: true,
                    minimum_length_elements: 1,
                },
                TassadarGeneralizedAbiMemoryRegion {
                    region_id: String::from("right_input"),
                    role: TassadarGeneralizedAbiMemoryRegionRole::Input,
                    pointer_param_index: 1,
                    length_param_index: 2,
                    element_width_bytes: 4,
                    signed: true,
                    minimum_length_elements: 1,
                },
            ],
            vec![
                String::from("panic_abort_loop_only"),
                String::from("core_pointer_arithmetic"),
            ],
            "multiple pointer-plus-length i32 inputs with one direct scalar i32 return are admitted under the widened generalized ABI family",
        )
    }

    #[must_use]
    pub fn sum_and_max_status_output() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::SumAndMaxStatusOutput,
            "heap_sum_article",
            "fixtures/tassadar/sources/tassadar_heap_sum_kernel.rs",
            "sum_and_max_into_buffer",
            "rust.heap_sum_article.result_code_output_buffer",
            "result_code_plus_output_buffer_i32",
            vec![
                TassadarGeneralizedAbiParamKind::PointerToI32,
                TassadarGeneralizedAbiParamKind::LengthI32,
                TassadarGeneralizedAbiParamKind::PointerToI32,
                TassadarGeneralizedAbiParamKind::LengthI32,
            ],
            vec![TassadarGeneralizedAbiResultKind::I32],
            vec![
                TassadarGeneralizedAbiMemoryRegion {
                    region_id: String::from("input_values"),
                    role: TassadarGeneralizedAbiMemoryRegionRole::Input,
                    pointer_param_index: 0,
                    length_param_index: 1,
                    element_width_bytes: 4,
                    signed: true,
                    minimum_length_elements: 1,
                },
                TassadarGeneralizedAbiMemoryRegion {
                    region_id: String::from("output_values"),
                    role: TassadarGeneralizedAbiMemoryRegionRole::Output,
                    pointer_param_index: 2,
                    length_param_index: 3,
                    element_width_bytes: 4,
                    signed: true,
                    minimum_length_elements: 2,
                },
            ],
            vec![
                String::from("caller_owned_output_buffers"),
                String::from("panic_abort_loop_only"),
            ],
            "caller-owned result-code-plus-output-buffer shapes are admitted under the widened generalized ABI family when the output buffer contract is explicit and bounded",
        )
    }

    #[must_use]
    pub fn pair_sum_and_diff_i32() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::PairSumAndDiffI32,
            "wider_numeric_pair_sum_and_diff",
            "fixtures/tassadar/sources/tassadar_wider_numeric_kernel.rs",
            "pair_sum_and_diff",
            "rust.wider_numeric_kernel.pair_sum_and_diff",
            "homogeneous_i32_pair_return",
            vec![
                TassadarGeneralizedAbiParamKind::I32,
                TassadarGeneralizedAbiParamKind::I32,
            ],
            vec![
                TassadarGeneralizedAbiResultKind::I32,
                TassadarGeneralizedAbiResultKind::I32,
            ],
            Vec::new(),
            vec![
                String::from("homogeneous_multi_value_returns"),
                String::from("panic_abort_loop_only"),
            ],
            "one homogeneous two-value i32 return shape is admitted under the widened numeric and data-layout ladder",
        )
    }

    #[must_use]
    pub fn sum_and_max_i64_status_output() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::SumAndMaxI64StatusOutput,
            "wider_numeric_i64_status_output",
            "fixtures/tassadar/sources/tassadar_wider_numeric_kernel.rs",
            "sum_and_max_i64_into_buffer",
            "rust.wider_numeric_kernel.i64_status_output",
            "result_code_plus_output_buffer_i64",
            vec![
                TassadarGeneralizedAbiParamKind::PointerToI32,
                TassadarGeneralizedAbiParamKind::LengthI32,
                TassadarGeneralizedAbiParamKind::PointerToI32,
                TassadarGeneralizedAbiParamKind::LengthI32,
            ],
            vec![TassadarGeneralizedAbiResultKind::I32],
            vec![
                TassadarGeneralizedAbiMemoryRegion {
                    region_id: String::from("input_values_i64"),
                    role: TassadarGeneralizedAbiMemoryRegionRole::Input,
                    pointer_param_index: 0,
                    length_param_index: 1,
                    element_width_bytes: 8,
                    signed: true,
                    minimum_length_elements: 1,
                },
                TassadarGeneralizedAbiMemoryRegion {
                    region_id: String::from("output_values_i64"),
                    role: TassadarGeneralizedAbiMemoryRegionRole::Output,
                    pointer_param_index: 2,
                    length_param_index: 3,
                    element_width_bytes: 8,
                    signed: true,
                    minimum_length_elements: 2,
                },
            ],
            vec![
                String::from("caller_owned_output_buffers"),
                String::from("i64_region_layouts"),
                String::from("panic_abort_loop_only"),
            ],
            "caller-owned status-code-plus-output-buffer shapes with 8-byte aligned i64 layouts are admitted under the widened numeric and data-layout ladder",
        )
    }

    #[must_use]
    pub fn multi_export_pair_sum() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::MultiExportPairSum,
            "multi_export_exact",
            "fixtures/tassadar/sources/tassadar_multi_export_kernel.rs",
            "pair_sum",
            "rust.multi_export_kernel.generalized_pair_sum",
            "bounded_multi_export_program_shape",
            Vec::new(),
            vec![TassadarGeneralizedAbiResultKind::I32],
            Vec::new(),
            vec![
                String::from("bounded_multi_export_program_shape"),
                String::from("panic_abort_loop_only"),
            ],
            "bounded multi-export program shapes are admitted when each export remains independently benchmarked and bound to the same source lineage",
        )
    }

    #[must_use]
    pub fn multi_export_local_double() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::MultiExportLocalDouble,
            "multi_export_exact",
            "fixtures/tassadar/sources/tassadar_multi_export_kernel.rs",
            "local_double",
            "rust.multi_export_kernel.generalized_local_double",
            "bounded_multi_export_program_shape",
            Vec::new(),
            vec![TassadarGeneralizedAbiResultKind::I32],
            Vec::new(),
            vec![
                String::from("bounded_multi_export_program_shape"),
                String::from("local_frame_helpers"),
                String::from("panic_abort_loop_only"),
            ],
            "bounded multi-export program shapes may use simple local-frame helpers without widening into arbitrary runtime support",
        )
    }

    #[must_use]
    pub fn unsupported_float_param() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::UnsupportedFloatParam,
            "param_abi_fixture",
            "fixtures/tassadar/sources/tassadar_param_abi_kernel.rs",
            "add_one_f32",
            "rust.param_abi_kernel.unsupported_float",
            "floating_point_param_abi",
            vec![TassadarGeneralizedAbiParamKind::F32],
            vec![TassadarGeneralizedAbiResultKind::I32],
            Vec::new(),
            vec![String::from("panic_abort_loop_only")],
            "floating-point parameter ABI shapes remain explicit refusals under the widened generalized ABI family",
        )
    }

    #[must_use]
    pub fn unsupported_multi_result() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::UnsupportedMultiResult,
            "wider_numeric_mixed_multi_result",
            "fixtures/tassadar/sources/tassadar_wider_numeric_kernel.rs",
            "pair_sum_and_diff",
            "rust.wider_numeric_kernel.unsupported_mixed_multi_result",
            "mixed_multi_value_return_abi",
            vec![TassadarGeneralizedAbiParamKind::I32],
            vec![
                TassadarGeneralizedAbiResultKind::I32,
                TassadarGeneralizedAbiResultKind::I64,
            ],
            Vec::new(),
            vec![String::from("panic_abort_loop_only")],
            "mixed-width multi-value return shapes remain explicit refusals under the widened numeric and data-layout ladder",
        )
    }

    #[must_use]
    pub fn unsupported_host_handle() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::UnsupportedHostHandle,
            "param_abi_fixture",
            "fixtures/tassadar/sources/tassadar_param_abi_kernel.rs",
            "use_host_handle",
            "rust.param_abi_kernel.unsupported_host_handle",
            "host_handle_or_callback_abi",
            vec![TassadarGeneralizedAbiParamKind::HostHandle],
            vec![TassadarGeneralizedAbiResultKind::I32],
            Vec::new(),
            vec![String::from("panic_abort_loop_only")],
            "host-handle, callback, and trait-object ABI shapes remain explicit refusals under the widened generalized ABI family",
        )
    }

    #[must_use]
    pub fn unsupported_returned_buffer() -> Self {
        Self::new(
            TassadarGeneralizedAbiFixtureId::UnsupportedReturnedBuffer,
            "heap_sum_article",
            "fixtures/tassadar/sources/tassadar_heap_sum_kernel.rs",
            "callee_allocated_buffer",
            "rust.heap_sum_article.unsupported_returned_buffer",
            "callee_allocated_returned_buffer",
            vec![TassadarGeneralizedAbiParamKind::PointerToI32],
            vec![TassadarGeneralizedAbiResultKind::BufferPointerAndLength],
            Vec::new(),
            vec![String::from("panic_abort_loop_only")],
            "callee-allocated returned-buffer conventions remain explicit refusals under the widened generalized ABI family",
        )
    }
}

#[must_use]
pub fn tassadar_generalized_abi_fixture_suite() -> Vec<TassadarGeneralizedAbiFixture> {
    vec![
        TassadarGeneralizedAbiFixture::pair_add_i32(),
        TassadarGeneralizedAbiFixture::pair_add_i64(),
        TassadarGeneralizedAbiFixture::dual_heap_dot_i32(),
        TassadarGeneralizedAbiFixture::sum_and_max_status_output(),
        TassadarGeneralizedAbiFixture::pair_sum_and_diff_i32(),
        TassadarGeneralizedAbiFixture::sum_and_max_i64_status_output(),
        TassadarGeneralizedAbiFixture::multi_export_pair_sum(),
        TassadarGeneralizedAbiFixture::multi_export_local_double(),
        TassadarGeneralizedAbiFixture::unsupported_float_param(),
        TassadarGeneralizedAbiFixture::unsupported_multi_result(),
        TassadarGeneralizedAbiFixture::unsupported_host_handle(),
        TassadarGeneralizedAbiFixture::unsupported_returned_buffer(),
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
        TassadarGeneralizedAbiFixtureId, TassadarGeneralizedAbiMemoryRegionRole,
        tassadar_generalized_abi_fixture_suite,
    };

    #[test]
    fn generalized_abi_fixture_suite_is_machine_legible() {
        let fixtures = tassadar_generalized_abi_fixture_suite();

        assert_eq!(fixtures.len(), 12);
        assert!(fixtures.iter().any(|fixture| {
            fixture.fixture_id == TassadarGeneralizedAbiFixtureId::DualHeapDotI32
                && fixture.memory_regions.len() == 2
        }));
        assert!(fixtures.iter().any(|fixture| {
            fixture.fixture_id == TassadarGeneralizedAbiFixtureId::SumAndMaxStatusOutput
                && fixture
                    .memory_regions
                    .iter()
                    .any(|region| region.role == TassadarGeneralizedAbiMemoryRegionRole::Output)
        }));
        assert!(fixtures.iter().any(|fixture| {
            fixture.fixture_id == TassadarGeneralizedAbiFixtureId::PairAddI64
                && fixture.param_kinds
                    == vec![
                        super::TassadarGeneralizedAbiParamKind::I64,
                        super::TassadarGeneralizedAbiParamKind::I64,
                    ]
        }));
    }
}
