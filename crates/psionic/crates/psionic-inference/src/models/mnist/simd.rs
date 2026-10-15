//! Native x86 convolution over eight adjacent output pixels per vector.
use rayon::prelude::*;
use std::arch::x86_64::*;

/// The caller checks AVX2 and FMA support. Every vector load fits the padded image.
#[target_feature(enable = "avx2,fma")]
pub(super) unsafe fn conv_relu(
    input: &[f32],
    channels: usize,
    width: usize,
    weights: &[f32],
    bias: &[f32],
    parallel: bool,
) -> Vec<f32> {
    let stride = width.div_ceil(8) * 8 + 4;
    let height = width + 4;
    let mut padded = vec![0.0; channels * height * stride];
    for c in 0..channels {
        for y in 0..width {
            padded[(c * height + y + 2) * stride + 2..(c * height + y + 2) * stride + 2 + width]
                .copy_from_slice(&input[(c * width + y) * width..(c * width + y + 1) * width]);
        }
    }
    let mut output = vec![0.0; bias.len() * width * width];
    let group = if bias.len() % 8 == 0 {
        8
    } else if bias.len() % 4 == 0 {
        4
    } else {
        1
    };
    let work = |block: usize, planes: &mut [f32]| {
        let oc = block * group;
        // SAFETY: CPU features were checked and this group owns complete output planes.
        unsafe {
            match group {
                8 => convolve_eight(
                    &padded,
                    channels,
                    width,
                    height,
                    stride,
                    weights,
                    oc,
                    &bias[oc..oc + 8],
                    planes,
                ),
                4 => convolve_four(
                    &padded,
                    channels,
                    width,
                    height,
                    stride,
                    weights,
                    oc,
                    &bias[oc..oc + 4],
                    planes,
                ),
                _ => convolve_plane(
                    &padded, channels, width, height, stride, weights, oc, bias[oc], planes,
                ),
            }
        }
    };
    if parallel {
        output
            .par_chunks_mut(group * width * width)
            .enumerate()
            .for_each(|(g, p)| work(g, p));
    } else {
        for (g, p) in output.chunks_mut(group * width * width).enumerate() {
            work(g, p);
        }
    }
    output
}

#[target_feature(enable = "avx2,fma")]
unsafe fn convolve_plane(
    input: &[f32],
    channels: usize,
    width: usize,
    height: usize,
    stride: usize,
    weights: &[f32],
    oc: usize,
    bias: f32,
    plane: &mut [f32],
) {
    for y in 0..width {
        for x in (0..width).step_by(8) {
            let mut sum = _mm256_setzero_ps();
            for c in 0..channels {
                for ky in 0..5 {
                    for kx in 0..5 {
                        let offset = (c * height + y + ky) * stride + x + kx;
                        let w = weights[((oc * channels + c) * 5 + ky) * 5 + kx];
                        // SAFETY: the right padding includes the entire final eight-pixel vector.
                        let pixels = unsafe { _mm256_loadu_ps(input.as_ptr().add(offset)) };
                        sum = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), sum);
                    }
                }
            }
            sum = _mm256_max_ps(
                _mm256_add_ps(sum, _mm256_set1_ps(bias)),
                _mm256_setzero_ps(),
            );
            let count = 8.min(width - x);
            if count == 8 {
                // SAFETY: a full vector fits this output row.
                unsafe {
                    _mm256_storeu_ps(plane.as_mut_ptr().add(y * width + x), sum);
                }
            } else {
                let mut tail = [0.0; 8];
                // SAFETY: the temporary holds eight f32 elements.
                unsafe {
                    _mm256_storeu_ps(tail.as_mut_ptr(), sum);
                }
                plane[y * width + x..y * width + x + count].copy_from_slice(&tail[..count]);
            }
        }
    }
}

#[target_feature(enable = "avx2,fma")]
unsafe fn convolve_four(
    input: &[f32],
    channels: usize,
    width: usize,
    height: usize,
    stride: usize,
    weights: &[f32],
    oc: usize,
    bias: &[f32],
    planes: &mut [f32],
) {
    for y in 0..width {
        for x in (0..width).step_by(8) {
            let mut a = _mm256_setzero_ps();
            let mut b = a;
            let mut c = a;
            let mut d = a;
            for channel in 0..channels {
                for ky in 0..5 {
                    for kx in 0..5 {
                        let offset = (channel * height + y + ky) * stride + x + kx;
                        let k = (channel * 5 + ky) * 5 + kx;
                        // SAFETY: padded rows include every vector and the model has four output channels here.
                        let pixels = unsafe { _mm256_loadu_ps(input.as_ptr().add(offset)) };
                        let w = unsafe { *weights.get_unchecked(oc * channels * 25 + k) };
                        a = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a);
                        let w = unsafe { *weights.get_unchecked((oc + 1) * channels * 25 + k) };
                        b = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), b);
                        let w = unsafe { *weights.get_unchecked((oc + 2) * channels * 25 + k) };
                        c = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), c);
                        let w = unsafe { *weights.get_unchecked((oc + 3) * channels * 25 + k) };
                        d = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), d);
                    }
                }
            }
            for (plane, sum) in [a, b, c, d].into_iter().enumerate() {
                let value = _mm256_max_ps(
                    _mm256_add_ps(sum, _mm256_set1_ps(bias[plane])),
                    _mm256_setzero_ps(),
                );
                let offset = plane * width * width + y * width + x;
                let count = 8.min(width - x);
                if count == 8 {
                    unsafe {
                        _mm256_storeu_ps(planes.as_mut_ptr().add(offset), value);
                    }
                } else {
                    let mut tail = [0.0; 8];
                    unsafe {
                        _mm256_storeu_ps(tail.as_mut_ptr(), value);
                    }
                    planes[offset..offset + count].copy_from_slice(&tail[..count]);
                }
            }
        }
    }
}

#[target_feature(enable = "avx2,fma")]
unsafe fn convolve_eight(
    input: &[f32],
    channels: usize,
    width: usize,
    height: usize,
    stride: usize,
    weights: &[f32],
    oc: usize,
    bias: &[f32],
    planes: &mut [f32],
) {
    for y in 0..width {
        for x in (0..width).step_by(8) {
            let mut a0 = _mm256_setzero_ps();
            let mut a1 = _mm256_setzero_ps();
            let mut a2 = _mm256_setzero_ps();
            let mut a3 = _mm256_setzero_ps();
            let mut a4 = _mm256_setzero_ps();
            let mut a5 = _mm256_setzero_ps();
            let mut a6 = _mm256_setzero_ps();
            let mut a7 = _mm256_setzero_ps();

            for channel in 0..channels {
                for ky in 0..5 {
                    for kx in 0..5 {
                        let offset = (channel * height + y + ky) * stride + x + kx;
                        let k = (channel * 5 + ky) * 5 + kx;
                        // SAFETY: the padded row contains the vector; all eight channel weights are present.
                        let pixels = unsafe { _mm256_loadu_ps(input.as_ptr().add(offset)) };
                        let w = unsafe { *weights.get_unchecked((oc + 0) * channels * 25 + k) };
                        a0 = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a0);
                        let w = unsafe { *weights.get_unchecked((oc + 1) * channels * 25 + k) };
                        a1 = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a1);
                        let w = unsafe { *weights.get_unchecked((oc + 2) * channels * 25 + k) };
                        a2 = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a2);
                        let w = unsafe { *weights.get_unchecked((oc + 3) * channels * 25 + k) };
                        a3 = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a3);
                        let w = unsafe { *weights.get_unchecked((oc + 4) * channels * 25 + k) };
                        a4 = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a4);
                        let w = unsafe { *weights.get_unchecked((oc + 5) * channels * 25 + k) };
                        a5 = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a5);
                        let w = unsafe { *weights.get_unchecked((oc + 6) * channels * 25 + k) };
                        a6 = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a6);
                        let w = unsafe { *weights.get_unchecked((oc + 7) * channels * 25 + k) };
                        a7 = _mm256_fmadd_ps(pixels, _mm256_set1_ps(w), a7);
                    }
                }
            }
            for (plane, sum) in [a0, a1, a2, a3, a4, a5, a6, a7].into_iter().enumerate() {
                let value = _mm256_max_ps(
                    _mm256_add_ps(sum, _mm256_set1_ps(bias[plane])),
                    _mm256_setzero_ps(),
                );
                let offset = plane * width * width + y * width + x;
                let count = 8.min(width - x);
                if count == 8 {
                    unsafe {
                        _mm256_storeu_ps(planes.as_mut_ptr().add(offset), value);
                    }
                } else {
                    let mut tail = [0.0; 8];
                    unsafe {
                        _mm256_storeu_ps(tail.as_mut_ptr(), value);
                    }
                    planes[offset..offset + count].copy_from_slice(&tail[..count]);
                }
            }
        }
    }
}
