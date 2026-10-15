//! Checked CUDA operations for device-resident bidirectional encoders.
use crate::clef_prefill::{f32s, int, span};
use crate::{CudaBuffer, CudaSubmission};
use psionic_runtime::RuntimeError;
use std::ffi::c_void;

unsafe extern "C" {
    fn psionic_encoder_embed(
        table: *const c_void,
        ids: *const c_void,
        out: *mut c_void,
        n: i32,
        h: i32,
        vocab: i32,
        zero: i32,
        scale: f32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_encoder_rope(
        qkv: *const c_void,
        freq: *const c_void,
        out: *mut c_void,
        n: i32,
        heads: i32,
        d: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_encoder_attention(
        qkv: *const c_void,
        mask: *const c_void,
        out: *mut c_void,
        n: i32,
        heads: i32,
        d: i32,
        radius: i32,
        queries: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_encoder_geglu(
        up: *const c_void,
        out: *mut c_void,
        n: i32,
        f: i32,
        stream: *mut c_void,
    ) -> i32;
}

impl CudaSubmission {
    /// Gathers uint8 token embeddings and dequantizes directly into device memory.
    #[allow(clippy::too_many_arguments)]
    pub fn encoder_embed(
        &mut self,
        table: &CudaBuffer,
        ids: &CudaBuffer,
        out: &CudaBuffer,
        n: usize,
        h: usize,
        vocab: usize,
        zero: u8,
        scale: f32,
    ) -> Result<(), RuntimeError> {
        encoder_dims(n, h)?;
        if vocab == 0 || vocab > 1_000_000 {
            return Err(RuntimeError::Backend(
                "Unsupported encoder vocabulary".into(),
            ));
        }
        let (table, ids, out) = (
            span(
                table,
                0,
                vocab
                    .checked_mul(h)
                    .ok_or_else(|| RuntimeError::Backend("Encoder table is too large".into()))?,
                1,
                "encoder table",
            )?,
            f32s(ids, n, "encoder ids")?,
            f32s(out, n * h, "encoder embedding")?,
        );
        // SAFETY: buffers have the checked extents; the kernel bounds-checks token IDs.
        let code = unsafe {
            psionic_encoder_embed(
                table,
                ids,
                out,
                int(n, "n")?,
                int(h, "h")?,
                int(vocab, "vocab")?,
                i32::from(zero),
                scale,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "encoder embedding")
    }
    /// Rotates and scales Q/K, and packs Q/K/V by head for bidirectional attention.
    pub fn encoder_rope(
        &mut self,
        qkv: &CudaBuffer,
        frequencies: &CudaBuffer,
        out: &CudaBuffer,
        n: usize,
        heads: usize,
        d: usize,
    ) -> Result<(), RuntimeError> {
        encoder_dims(n, heads.checked_mul(d).unwrap_or(usize::MAX))?;
        if d == 0 || d % 2 != 0 {
            return Err(RuntimeError::Backend(
                "Unsupported rotary dimensions".into(),
            ));
        }
        let (qkv, freq, out) = (
            f32s(qkv, 3 * n * heads * d, "encoder qkv")?,
            f32s(frequencies, d / 2, "encoder frequencies")?,
            f32s(out, 3 * n * heads * d, "encoder rotated")?,
        );
        let code = unsafe {
            psionic_encoder_rope(
                qkv,
                freq,
                out,
                int(n, "n")?,
                int(heads, "heads")?,
                int(d, "d")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "encoder rotary")
    }
    /// Bidirectional float32 attention with a key mask and an inclusive optional local radius.
    #[allow(clippy::too_many_arguments)]
    pub fn encoder_attention(
        &mut self,
        qkv: &CudaBuffer,
        mask: &CudaBuffer,
        out: &CudaBuffer,
        n: usize,
        heads: usize,
        d: usize,
        radius: Option<usize>,
        queries: usize,
    ) -> Result<(), RuntimeError> {
        encoder_dims(n, heads.checked_mul(d).unwrap_or(usize::MAX))?;
        if queries == 0 || queries > n || n == 0 || n > 512 || heads == 0 || d == 0 || d % 2 != 0 {
            return Err(RuntimeError::Backend(
                "Unsupported encoder attention dimensions".into(),
            ));
        }
        let (qkv, mask, out) = (
            f32s(qkv, 3 * n * heads * d, "encoder qkv")?,
            f32s(mask, n, "encoder mask")?,
            f32s(out, n * heads * d, "encoder context")?,
        );
        let code = unsafe {
            psionic_encoder_attention(
                qkv,
                mask,
                out,
                int(n, "n")?,
                int(heads, "heads")?,
                int(d, "d")?,
                radius.map(|r| int(r, "radius")).transpose()?.unwrap_or(-1),
                int(queries, "queries")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "encoder attention")
    }
    /// Exact-erf GELU gate multiplied by the second half of each projection row.
    pub fn encoder_geglu(
        &mut self,
        up: &CudaBuffer,
        out: &CudaBuffer,
        n: usize,
        f: usize,
    ) -> Result<(), RuntimeError> {
        encoder_dims(n, f)?;
        let (up, out) = (
            f32s(up, 2 * n * f, "encoder up")?,
            f32s(out, n * f, "encoder gate")?,
        );
        let code = unsafe {
            psionic_encoder_geglu(
                up,
                out,
                int(n, "n")?,
                int(f, "f")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "encoder GeGLU")
    }
}

fn encoder_dims(n: usize, width: usize) -> Result<(), RuntimeError> {
    if n == 0 || n > 512 || width == 0 || width > 16384 {
        Err(RuntimeError::Backend(
            "Unsupported encoder dimensions".into(),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn encoder_dimensions_reject_empty_and_excessive_ranges() {
        for (n, w) in [
            (0, 64),
            (513, 64),
            (1, 0),
            (1, 16385),
            (usize::MAX, usize::MAX),
        ] {
            assert!(encoder_dims(n, w).is_err());
        }
        assert!(encoder_dims(512, 16384).is_ok());
    }
}

unsafe extern "C" {
    fn psionic_encoder_softmax(
        scores: *mut c_void,
        mask: *const c_void,
        n: i32,
        heads: i32,
        radius: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_encoder_merge(
        packed: *const c_void,
        out: *mut c_void,
        n: i32,
        heads: i32,
        d: i32,
        stream: *mut c_void,
    ) -> i32;
}
impl CudaSubmission {
    /// Applies key padding and an inclusive local mask before float32 row softmax.
    pub fn encoder_softmax(
        &mut self,
        scores: &CudaBuffer,
        mask: &CudaBuffer,
        n: usize,
        heads: usize,
        radius: Option<usize>,
    ) -> Result<(), RuntimeError> {
        encoder_dims(n, heads)?;
        let (scores, mask) = (
            f32s(scores, heads * n * n, "encoder scores")?,
            f32s(mask, n, "encoder mask")?,
        );
        let code = unsafe {
            psionic_encoder_softmax(
                scores,
                mask,
                int(n, "n")?,
                int(heads, "heads")?,
                radius.map(|r| int(r, "radius")).transpose()?.unwrap_or(-1),
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "encoder softmax")
    }
    /// Packs per-head attention output into token-major rows.
    pub fn encoder_merge(
        &mut self,
        packed: &CudaBuffer,
        out: &CudaBuffer,
        n: usize,
        heads: usize,
        d: usize,
    ) -> Result<(), RuntimeError> {
        encoder_dims(n, heads.checked_mul(d).unwrap_or(usize::MAX))?;
        let (packed, out) = (
            f32s(packed, n * heads * d, "encoder packed context")?,
            f32s(out, n * heads * d, "encoder context")?,
        );
        let code = unsafe {
            psionic_encoder_merge(
                packed,
                out,
                int(n, "n")?,
                int(heads, "heads")?,
                int(d, "d")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "encoder merge")
    }
}
