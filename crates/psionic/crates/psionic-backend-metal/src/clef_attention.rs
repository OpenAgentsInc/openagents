//! Checked shapes and implementation selection for Clef attention.
//!
//! This module needs no Metal device. Both attention implementations use the
//! same input layouts and absolute query positions.

use std::str::FromStr;

/// Query rows computed by one fused threadgroup.
pub const CLEF_FUSED_QUERY_TILE: usize = 16;
/// Cache rows processed by one fused key tile.
pub const CLEF_FUSED_KEY_TILE: usize = 32;
/// Threads dispatched in one fused threadgroup.
pub const CLEF_FUSED_THREADS: usize = 128;

/// The implementation selected before loading a Clef Metal model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClefMetalAttention {
    /// Separate score, softmax, and value-product dispatches.
    #[default]
    Staged,
    /// Experimental fused attention using TensorOps.
    FusedTensorOps,
}

impl FromStr for ClefMetalAttention {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "staged" => Ok(Self::Staged),
            "fused-tensorops" => Ok(Self::FusedTensorOps),
            _ => Err(String::from(
                "clef metal attention must be staged or fused-tensorops",
            )),
        }
    }
}

/// Validated dimensions, buffer sizes, and dispatch counts for attention.
///
/// Queries and output use `[n, heads, 256]`; each cache uses
/// `[first + n, kv_heads, 256]`. `first` is the absolute position of the
/// first query. A zero-query plan accesses no buffers and dispatches no work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefAttentionPlan {
    /// Number of query rows.
    pub n: u32,
    /// Number of query heads.
    pub heads: u32,
    /// Number of key and value heads.
    pub kv_heads: u32,
    /// Absolute position of the first query.
    pub first: u32,
    /// Number of cache rows, including the current queries.
    pub keys: usize,
    /// Required bytes in the f16 query buffer.
    pub query_bytes: usize,
    /// Required bytes in each f16 key or value cache.
    pub key_value_bytes: usize,
    /// Required bytes in the f32 output buffer.
    pub output_bytes: usize,
    /// Required f32 scratch bytes for the staged score matrix.
    pub score_bytes: usize,
    /// Required f16 scratch bytes for the staged probability matrix.
    pub probability_bytes: usize,
    /// Fused query threadgroups per query head.
    pub fused_query_groups: usize,
}

impl ClefAttentionPlan {
    /// Checks the shape and every buffer size before any GPU work is encoded.
    ///
    /// TensorOps uses signed 32-bit extents and row strides. The shader's
    /// unsigned arguments therefore must also fit in that narrower range.
    pub fn new(
        n: usize,
        heads: usize,
        kv_heads: usize,
        dim: usize,
        first: usize,
    ) -> Result<Self, String> {
        if dim != 256 {
            return Err(String::from(
                "clef metal attention requires head dimension 256",
            ));
        }
        if heads == 0 || kv_heads == 0 || heads % kv_heads != 0 {
            return Err(String::from(
                "clef metal attention requires positive heads divisible by key/value heads",
            ));
        }
        let keys = first
            .checked_add(n)
            .ok_or_else(|| String::from("clef metal attention cache length overflows usize"))?;
        let n_arg = tensor_extent(n, "query count")?;
        let heads_arg = tensor_extent(heads, "query heads")?;
        let kv_heads_arg = tensor_extent(kv_heads, "key/value heads")?;
        let first_arg = tensor_extent(first, "first query position")?;
        tensor_extent(keys, "cache length")?;
        let query_stride = checked_product(&[heads, dim], "query row stride")?;
        let cache_stride = checked_product(&[kv_heads, dim], "cache row stride")?;
        tensor_extent(query_stride, "query row stride")?;
        tensor_extent(cache_stride, "cache row stride")?;

        let (query_bytes, key_value_bytes, output_bytes, score_bytes, probability_bytes) = if n == 0
        {
            (0, 0, 0, 0, 0)
        } else {
            (
                checked_product(&[n, query_stride, 2], "query bytes")?,
                checked_product(&[keys, cache_stride, 2], "key/value bytes")?,
                checked_product(&[n, query_stride, 4], "output bytes")?,
                checked_product(&[n, keys, 4], "score bytes")?,
                checked_product(&[n, keys, 2], "probability bytes")?,
            )
        };
        Ok(Self {
            n: n_arg,
            heads: heads_arg,
            kv_heads: kv_heads_arg,
            first: first_arg,
            keys,
            query_bytes,
            key_value_bytes,
            output_bytes,
            score_bytes,
            probability_bytes,
            fused_query_groups: n.div_ceil(CLEF_FUSED_QUERY_TILE),
        })
    }
}

fn tensor_extent(value: usize, name: &str) -> Result<u32, String> {
    if value > i32::MAX as usize {
        return Err(format!("clef metal attention {name} exceeds i32"));
    }
    Ok(value as u32)
}

fn checked_product(values: &[usize], name: &str) -> Result<usize, String> {
    values.iter().try_fold(1usize, |product, value| {
        product
            .checked_mul(*value)
            .ok_or_else(|| format!("clef metal attention {name} overflows usize"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_defaults_to_staged_and_requires_an_exact_name() {
        assert_eq!(ClefMetalAttention::default(), ClefMetalAttention::Staged);
        assert_eq!("staged".parse(), Ok(ClefMetalAttention::Staged));
        assert_eq!(
            "fused-tensorops".parse(),
            Ok(ClefMetalAttention::FusedTensorOps)
        );
        for value in ["", "fused", "1", "FUSED-TENSOROPS", " staged", "staged "] {
            assert!(value.parse::<ClefMetalAttention>().is_err(), "{value:?}");
        }
    }

    #[test]
    fn grouped_heads_have_separate_query_and_cache_strides() {
        for (heads, kv_heads) in [(1, 1), (6, 2), (12, 3), (16, 2)] {
            let plan = ClefAttentionPlan::new(17, heads, kv_heads, 256, 63).unwrap();
            assert_eq!(plan.keys, 80);
            assert_eq!(plan.first, 63);
            assert_eq!(plan.query_bytes, 17 * heads * 256 * 2);
            assert_eq!(plan.key_value_bytes, 80 * kv_heads * 256 * 2);
            assert_eq!(plan.output_bytes, 17 * heads * 256 * 4);
            assert_eq!(plan.score_bytes, 17 * 80 * 4);
            assert_eq!(plan.probability_bytes, 17 * 80 * 2);
            assert_eq!(plan.fused_query_groups, 2);
            let group_size = plan.heads / plan.kv_heads;
            assert_eq!((plan.heads - 1) / group_size, plan.kv_heads - 1);
        }
    }

    #[test]
    fn invalid_head_shapes_are_rejected_even_for_empty_requests() {
        for n in [0, 1] {
            for (heads, kv_heads, dim) in [
                (0, 1, 256),
                (1, 0, 256),
                (3, 2, 256),
                (2, 4, 256),
                (4, 2, 128),
            ] {
                assert!(ClefAttentionPlan::new(n, heads, kv_heads, dim, 0).is_err());
            }
        }
    }

    #[test]
    fn empty_request_accesses_no_buffers_or_threadgroups() {
        let plan = ClefAttentionPlan::new(0, 12, 3, 256, 1025).unwrap();
        assert_eq!(plan.keys, 1025);
        assert_eq!(plan.first, 1025);
        assert_eq!(plan.query_bytes, 0);
        assert_eq!(plan.key_value_bytes, 0);
        assert_eq!(plan.output_bytes, 0);
        assert_eq!(plan.score_bytes, 0);
        assert_eq!(plan.probability_bytes, 0);
        assert_eq!(plan.fused_query_groups, 0);
    }

    #[test]
    fn partial_query_tiles_and_absolute_cache_positions_are_kept() {
        for first in [0, 31, 32, 63, 64, 65, 2049] {
            for (n, groups) in [
                (1, 1),
                (15, 1),
                (16, 1),
                (17, 2),
                (31, 2),
                (32, 2),
                (33, 3),
                (65, 5),
            ] {
                let plan = ClefAttentionPlan::new(n, 12, 3, 256, first).unwrap();
                assert_eq!(plan.keys, first + n);
                assert_eq!(plan.fused_query_groups, groups);
                assert_eq!(plan.key_value_bytes, (first + n) * 3 * 256 * 2);
            }
        }
    }

    #[test]
    fn rejects_position_overflow_and_signed_tensor_extent_overflow() {
        assert!(ClefAttentionPlan::new(1, 1, 1, 256, usize::MAX).is_err());
        let too_large = i32::MAX as usize + 1;
        for (n, first) in [(too_large, 0), (0, too_large), (1, i32::MAX as usize)] {
            assert!(ClefAttentionPlan::new(n, 1, 1, 256, first).is_err());
        }
        let too_many_heads = i32::MAX as usize / 256 + 1;
        assert!(ClefAttentionPlan::new(1, too_many_heads, 1, 256, 0).is_err());
        assert!(ClefAttentionPlan::new(1, too_many_heads, too_many_heads, 256, 0).is_err());
        assert!(checked_product(&[usize::MAX, 2], "test bytes").is_err());
    }

    #[test]
    fn fused_path_can_avoid_192_mib_of_staged_scratch_at_16k() {
        let plan = ClefAttentionPlan::new(2048, 16, 2, 256, 14336).unwrap();
        assert_eq!(plan.keys, 16384);
        assert_eq!(plan.fused_query_groups, 128);
        assert_eq!(plan.score_bytes, 128 * 1024 * 1024);
        assert_eq!(plan.probability_bytes, 64 * 1024 * 1024);
        assert_eq!(plan.score_bytes + plan.probability_bytes, 192 * 1024 * 1024);
    }
}
