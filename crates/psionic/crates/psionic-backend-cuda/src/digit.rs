//! Checked CUDA operations for the pinned handwritten-digit graph.
use crate::clef_prefill::{f32s, int};
use crate::{CudaBuffer, CudaSubmission};
use psionic_runtime::RuntimeError;
use std::ffi::c_void;

unsafe extern "C" {
    fn psionic_digit_conv_pool(
        input: *const c_void,
        weights: *const c_void,
        bias: *const c_void,
        out: *mut c_void,
        channels: i32,
        width: i32,
        outputs: i32,
        pool: i32,
        stream: *mut c_void,
    ) -> i32;
    fn psionic_digit_head(
        input: *const c_void,
        weights: *const c_void,
        bias: *const c_void,
        out: *mut c_void,
        stream: *mut c_void,
    ) -> i32;
}
impl CudaSubmission {
    /// Five-by-five SAME convolution, ReLU, then floor max pooling, all in float32.
    #[allow(clippy::too_many_arguments)]
    pub fn digit_conv_pool(
        &mut self,
        input: &CudaBuffer,
        weights: &CudaBuffer,
        bias: &CudaBuffer,
        out: &CudaBuffer,
        channels: usize,
        width: usize,
        outputs: usize,
        pool: usize,
    ) -> Result<(), RuntimeError> {
        if !(1..=64).contains(&channels)
            || !(1..=64).contains(&outputs)
            || !(1..=128).contains(&width)
            || !(1..=4).contains(&pool)
            || pool > width
        {
            return Err(RuntimeError::Backend(
                "Unsupported digit convolution dimensions".into(),
            ));
        }
        let next = width / pool;
        let (input, weights, bias, out) = (
            f32s(input, channels * width * width, "digit input")?,
            f32s(weights, outputs * channels * 25, "digit weights")?,
            f32s(bias, outputs, "digit bias")?,
            f32s(out, outputs * next * next, "digit pooled output")?,
        );
        // SAFETY: dimensions and every input/output extent were checked above.
        let code = unsafe {
            psionic_digit_conv_pool(
                input,
                weights,
                bias,
                out,
                int(channels, "channels")?,
                int(width, "width")?,
                int(outputs, "outputs")?,
                int(pool, "pool")?,
                self.platform.raw_stream()?,
            )
        };
        self.clef_launch(code, "digit convolution and pooling")
    }
    /// Dense 256-to-10 float32 digit logits, including bias.
    pub fn digit_head(
        &mut self,
        input: &CudaBuffer,
        weights: &CudaBuffer,
        bias: &CudaBuffer,
        out: &CudaBuffer,
    ) -> Result<(), RuntimeError> {
        let (input, weights, bias, out) = (
            f32s(input, 256, "digit features")?,
            f32s(weights, 256 * 10, "digit dense weights")?,
            f32s(bias, 10, "digit dense bias")?,
            f32s(out, 10, "digit logits")?,
        );
        // SAFETY: the fixed graph's complete buffer extents were checked above.
        let code =
            unsafe { psionic_digit_head(input, weights, bias, out, self.platform.raw_stream()?) };
        self.clef_launch(code, "digit dense head")
    }
}
