//! Device-independent dispatch planning for Clef's f16 weight projections.

use std::str::FromStr;

/// Threads used by the four-SIMD-group TensorOps projection kernels.
pub const CLEF_GEMM_THREADS: usize = 128;

/// Projection tiling selected once, before loading the model's weights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClefMetalGemm {
    /// Use the original 64-token by 64-output tile for every weight.
    #[default]
    Fixed64,
    /// Experimental tiles for the six Clef-Flash trunk weight shapes.
    WeightShapes,
}

impl FromStr for ClefMetalGemm {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "fixed-64" => Ok(Self::Fixed64),
            "weight-shapes" => Ok(Self::WeightShapes),
            _ => Err(String::from(
                "clef metal GEMM must be fixed-64 or weight-shapes",
            )),
        }
    }
}

/// Checked buffers and dispatch dimensions for `out[n,m] (+)= x[n,k] W[m,k]^T`.
///
/// Input and weight values are f16; output values are f32. Token tiles always
/// have 64 rows. Weight selection does not depend on chunk length or offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefGemmPlan {
    /// Metal kernel name, including its residual-accumulation variant.
    pub kernel: &'static str,
    /// Output columns covered by a threadgroup.
    pub output_tile: usize,
    /// Output-column and token-row threadgroup counts.
    pub groups: [usize; 2],
    /// The shader's padded `uint3` arguments.
    pub nmk: [u32; 4],
    /// Byte offset into the input buffer.
    pub input_offset_bytes: usize,
    /// Byte offset into the output buffer.
    pub output_offset_bytes: usize,
    /// Input bytes accessed after the input offset.
    pub input_bytes: usize,
    /// Weight bytes accessed from the start of the weight buffer.
    pub weight_bytes: usize,
    /// Output bytes accessed after the output offset.
    pub output_bytes: usize,
}

impl ClefGemmPlan {
    /// Validates tensor extents, byte sizes, and offsets before encoding work.
    /// Offsets are in elements. A zero-token plan accesses no buffers.
    pub fn new(
        mode: ClefMetalGemm,
        n: usize,
        m: usize,
        k: usize,
        x_offset: usize,
        out_offset: usize,
        accumulate: bool,
    ) -> Result<Self, String> {
        if m == 0 || k == 0 {
            return Err(String::from(
                "clef metal GEMM weight dimensions must be positive",
            ));
        }
        let nmk = [extent(n)?, extent(m)?, extent(k)?, 0];
        let output_tile = match (mode, m, k) {
            (ClefMetalGemm::WeightShapes, 32, 4096) => 32,
            (ClefMetalGemm::WeightShapes, 4096 | 8192 | 12288, 4096)
            | (ClefMetalGemm::WeightShapes, 4096, 12288) => 128,
            // The 1024-column key/value projections and unknown checkpoints
            // keep the original tile until a measured comparison supports them.
            _ => 64,
        };
        let kernel = match (output_tile, accumulate) {
            (32, false) => "clef_gemm_64x32",
            (32, true) => "clef_gemm_64x32_accumulate",
            (128, false) => "clef_gemm_64x128",
            (128, true) => "clef_gemm_64x128_accumulate",
            (_, false) => "clef_gemm",
            (_, true) => "clef_gemm_accumulate",
        };
        let (input_offset_bytes, output_offset_bytes, input_bytes, weight_bytes, output_bytes) =
            if n == 0 {
                (0, 0, 0, 0, 0)
            } else {
                (
                    product(&[x_offset, 2])?,
                    product(&[out_offset, 4])?,
                    product(&[n, k, 2])?,
                    product(&[m, k, 2])?,
                    product(&[n, m, 4])?,
                )
            };
        for (offset, bytes) in [
            (input_offset_bytes, input_bytes),
            (output_offset_bytes, output_bytes),
        ] {
            offset
                .checked_add(bytes)
                .ok_or_else(|| String::from("clef metal GEMM buffer range overflows usize"))?;
        }
        Ok(Self {
            kernel,
            output_tile,
            groups: if n == 0 {
                [0, 0]
            } else {
                [m.div_ceil(output_tile), n.div_ceil(64)]
            },
            nmk,
            input_offset_bytes,
            output_offset_bytes,
            input_bytes,
            weight_bytes,
            output_bytes,
        })
    }
}

fn extent(value: usize) -> Result<u32, String> {
    if value > i32::MAX as usize {
        Err(String::from("clef metal GEMM tensor extent exceeds i32"))
    } else {
        Ok(value as u32)
    }
}

fn product(values: &[usize]) -> Result<usize, String> {
    values.iter().try_fold(1usize, |total, value| {
        total
            .checked_mul(*value)
            .ok_or_else(|| String::from("clef metal GEMM byte count overflows usize"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(mode: ClefMetalGemm, n: usize, m: usize, k: usize, accumulate: bool) -> ClefGemmPlan {
        ClefGemmPlan::new(mode, n, m, k, 0, 0, accumulate).unwrap()
    }

    #[test]
    fn default_and_exact_setting_names() {
        assert_eq!(ClefMetalGemm::default(), ClefMetalGemm::Fixed64);
        assert_eq!("fixed-64".parse(), Ok(ClefMetalGemm::Fixed64));
        assert_eq!("weight-shapes".parse(), Ok(ClefMetalGemm::WeightShapes));
        for value in [
            "",
            "auto",
            "1",
            "WEIGHT-SHAPES",
            " weight-shapes",
            "fixed-64 ",
        ] {
            assert!(value.parse::<ClefMetalGemm>().is_err());
        }
    }

    #[test]
    fn every_real_weight_shape_selects_the_expected_tile_in_both_modes() {
        for (m, k, tile) in [
            (32, 4096, 32),
            (1024, 4096, 64),
            (4096, 4096, 128),
            (8192, 4096, 128),
            (12288, 4096, 128),
            (4096, 12288, 128),
        ] {
            for n in [1, 33, 63, 64, 65, 1078, 1866, 2048] {
                for accumulate in [false, true] {
                    let tuned = plan(ClefMetalGemm::WeightShapes, n, m, k, accumulate);
                    let fixed = plan(ClefMetalGemm::Fixed64, n, m, k, accumulate);
                    assert_eq!(tuned.output_tile, tile);
                    assert_eq!(fixed.output_tile, 64);
                    assert_eq!(tuned.groups, [m.div_ceil(tile), n.div_ceil(64)]);
                    assert_eq!(tuned.input_bytes, fixed.input_bytes);
                    assert_eq!(tuned.weight_bytes, fixed.weight_bytes);
                    assert_eq!(tuned.output_bytes, fixed.output_bytes);
                    assert_eq!(tuned.kernel.ends_with("_accumulate"), accumulate);
                    assert_eq!(fixed.kernel.ends_with("_accumulate"), accumulate);
                    assert_eq!(
                        tuned.kernel,
                        match tile {
                            32 if accumulate => "clef_gemm_64x32_accumulate",
                            32 => "clef_gemm_64x32",
                            128 if accumulate => "clef_gemm_64x128_accumulate",
                            128 => "clef_gemm_64x128",
                            _ => fixed.kernel,
                        }
                    );
                }
            }
        }
    }

    #[test]
    fn unknown_weights_use_the_original_kernel_including_output_edges() {
        for (m, k) in [
            (31, 4096),
            (32, 4095),
            (33, 4096),
            (4096, 2048),
            (12289, 4096),
        ] {
            for accumulate in [false, true] {
                let tuned = plan(ClefMetalGemm::WeightShapes, 65, m, k, accumulate);
                let fixed = plan(ClefMetalGemm::Fixed64, 65, m, k, accumulate);
                assert_eq!(tuned, fixed);
                assert_eq!(tuned.groups, [m.div_ceil(64), 2]);
            }
        }
    }

    #[test]
    fn offsets_are_element_based_and_sizes_do_not_include_tile_padding() {
        let plan =
            ClefGemmPlan::new(ClefMetalGemm::WeightShapes, 65, 32, 4096, 9, 17, true).unwrap();
        assert_eq!(plan.nmk, [65, 32, 4096, 0]);
        assert_eq!(plan.input_offset_bytes, 18);
        assert_eq!(plan.output_offset_bytes, 68);
        assert_eq!(plan.input_bytes, 65 * 4096 * 2);
        assert_eq!(plan.weight_bytes, 32 * 4096 * 2);
        assert_eq!(plan.output_bytes, 65 * 32 * 4);
        assert_eq!(plan.groups, [1, 2]);
    }

    #[test]
    fn zero_tokens_need_no_buffer_access_or_dispatch() {
        let plan = ClefGemmPlan::new(
            ClefMetalGemm::WeightShapes,
            0,
            32,
            4096,
            usize::MAX,
            usize::MAX,
            false,
        )
        .unwrap();
        assert_eq!(plan.groups, [0, 0]);
        assert_eq!((plan.input_offset_bytes, plan.output_offset_bytes), (0, 0));
        assert_eq!(
            (plan.input_bytes, plan.weight_bytes, plan.output_bytes),
            (0, 0, 0)
        );
    }

    #[test]
    fn rejects_invalid_extents_and_overflow_before_buffer_binding() {
        for (n, m, k) in [
            (1, 0, 4096),
            (1, 32, 0),
            (usize::MAX, 32, 4096),
            (1, usize::MAX, 4096),
            (1, 32, usize::MAX),
        ] {
            assert!(ClefGemmPlan::new(ClefMetalGemm::WeightShapes, n, m, k, 0, 0, false).is_err());
        }
        for (x_offset, out_offset) in [
            (usize::MAX, 0),
            (0, usize::MAX),
            (usize::MAX / 2, 0),
            (0, usize::MAX / 4),
        ] {
            assert!(
                ClefGemmPlan::new(
                    ClefMetalGemm::WeightShapes,
                    1,
                    32,
                    4096,
                    x_offset,
                    out_offset,
                    false
                )
                .is_err()
            );
        }
    }
}
