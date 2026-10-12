//! Checked shapes for the experimental blockwise Clef DeltaNet scan.

use std::str::FromStr;

/// Tokens processed together by the parallel scan.
pub const CLEF_DELTA_BLOCK: usize = 32;
/// Threads in each preparation or recurrent threadgroup.
pub const CLEF_DELTA_THREADS: usize = 128;
/// Padded triangular system/beta, values, causal QK, and decays, in f32 elements.
pub const CLEF_DELTA_BLOCK_ELEMENTS: usize = 2 * 32 * 128 + 32 * 32 + 2 * 32;
/// Maximum additional scratch for one scan.
pub const CLEF_DELTA_MAX_WORKSPACE_BYTES: usize = 256 * 1024 * 1024;

/// The recurrent scan selected before loading a model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClefMetalDelta {
    /// The existing per-token recurrence.
    #[default]
    Sequential,
    /// Experimental block32 recurrence using f32 TensorOps products.
    BlockTensorOps,
}

impl FromStr for ClefMetalDelta {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "sequential" => Ok(Self::Sequential),
            "block-tensorops" => Ok(Self::BlockTensorOps),
            _ => Err(String::from(
                "clef metal delta must be sequential or block-tensorops",
            )),
        }
    }
}

/// Validated tensor extents, dispatch counts, and required buffer lengths.
///
/// All buffers contain f32 values. Query and key use `[n, key_heads, 128]`;
/// values start at `value_offset` in each convolution row. State uses
/// `[value_heads, 128, 128]`, ordered by value coordinate then key coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClefDeltaPlan {
    pub n: u32,
    pub key_heads: u32,
    pub value_heads: u32,
    pub v_head_reordered: u32,
    pub conv_width: u32,
    pub value_offset: u32,
    pub blocks: usize,
    pub query_key_bytes: usize,
    pub conv_bytes: usize,
    pub decay_beta_bytes: usize,
    pub state_bytes: usize,
    pub output_bytes: usize,
    pub workspace_bytes: usize,
}

impl ClefDeltaPlan {
    /// Checks all tensor extents and arithmetic before dispatch.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        n: usize,
        key_heads: usize,
        value_heads: usize,
        dim: usize,
        v_head_reordered: bool,
        conv_width: usize,
        value_offset: usize,
    ) -> Result<Self, String> {
        if dim != 128 || key_heads == 0 || value_heads == 0 || value_heads % key_heads != 0 {
            return Err(String::from(
                "clef block delta requires dimension 128 and positive value heads divisible by key heads",
            ));
        }
        let key_stride = product(&[key_heads, dim])?;
        let value_stride = product(&[value_heads, dim])?;
        let value_end = value_offset
            .checked_add(value_stride)
            .ok_or("clef block delta value range overflows")?;
        if value_end > conv_width {
            return Err(String::from(
                "clef block delta values exceed the convolution row",
            ));
        }
        extent(key_stride)?;
        extent(value_stride)?;
        let mut plan = Self {
            n: extent(n)?,
            key_heads: extent(key_heads)?,
            value_heads: extent(value_heads)?,
            v_head_reordered: u32::from(v_head_reordered),
            conv_width: extent(conv_width)?,
            value_offset: extent(value_offset)?,
            blocks: n.div_ceil(CLEF_DELTA_BLOCK),
            query_key_bytes: 0,
            conv_bytes: 0,
            decay_beta_bytes: 0,
            state_bytes: 0,
            output_bytes: 0,
            workspace_bytes: 0,
        };
        if n != 0 {
            plan.query_key_bytes = product(&[n, key_stride, 4])?;
            plan.conv_bytes = product(&[n, conv_width, 4])?;
            plan.decay_beta_bytes = product(&[n, value_heads, 4])?;
            plan.state_bytes = product(&[value_heads, dim, dim, 4])?;
            plan.output_bytes = product(&[n, value_stride, 4])?;
            plan.workspace_bytes = Self::workspace_bytes(n, value_heads)?;
        }
        Ok(plan)
    }

    /// Additional per-request scratch, reused across layers.
    pub fn workspace_bytes(n: usize, value_heads: usize) -> Result<usize, String> {
        extent(n)?;
        extent(value_heads)?;
        if value_heads == 0 {
            return Err(String::from(
                "clef block delta requires positive value heads",
            ));
        }
        let bytes = product(&[
            n.div_ceil(CLEF_DELTA_BLOCK),
            value_heads,
            CLEF_DELTA_BLOCK_ELEMENTS,
            4,
        ])?;
        if bytes > CLEF_DELTA_MAX_WORKSPACE_BYTES {
            return Err(String::from("clef block delta scratch exceeds 256 MiB"));
        }
        Ok(bytes)
    }
}

fn extent(value: usize) -> Result<u32, String> {
    if value > i32::MAX as usize {
        Err(String::from("clef block delta tensor extent exceeds i32"))
    } else {
        Ok(value as u32)
    }
}

fn product(values: &[usize]) -> Result<usize, String> {
    values.iter().try_fold(1usize, |total, &value| {
        total
            .checked_mul(value)
            .ok_or_else(|| String::from("clef block delta size overflows"))
    })
}

#[cfg(test)]
pub(crate) mod tests;
