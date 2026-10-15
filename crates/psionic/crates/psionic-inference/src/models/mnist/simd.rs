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
    rows: usize,
    buffers: &crate::buffers::Buffers,
    pool_two: bool,
) -> Vec<f32> {
    let stride = width.div_ceil(16) * 16 + 4;
    let height = width + 4;
    let mut padded = buffers.take(channels * height * stride);
    padded.fill(0.0);
    for c in 0..channels {
        for y in 0..width {
            padded[(c * height + y + 2) * stride + 2..(c * height + y + 2) * stride + 2 + width]
                .copy_from_slice(&input[(c * width + y) * width..(c * width + y + 1) * width]);
        }
    }
    assert!(rows > 0 && rows <= width);
    assert!(!pool_two || (width == 28 && rows == 28 && bias.len() % 4 == 0));
    let plane_len = if pool_two {
        rows * width / 4
    } else {
        rows * width
    };
    let mut output = buffers.take(bias.len() * plane_len);
    let group = if bias.len() % 4 == 0 { 4 } else { 1 };
    let work = |block: usize, planes: &mut [f32]| {
        let oc = block * group;
        // SAFETY: CPU features were checked and this group owns complete output planes.
        unsafe {
            match group {
                4 => convolve_four_wide(
                    &padded,
                    channels,
                    width,
                    rows,
                    height,
                    stride,
                    weights,
                    oc,
                    &bias[oc..oc + 4],
                    planes,
                    pool_two,
                ),
                _ => convolve_plane(
                    &padded, channels, width, rows, height, stride, weights, oc, bias[oc], planes,
                ),
            }
        }
    };
    if parallel {
        output
            .par_chunks_mut(group * plane_len)
            .enumerate()
            .for_each(|(g, p)| work(g, p));
    } else {
        for (g, p) in output.chunks_mut(group * plane_len).enumerate() {
            work(g, p);
        }
    }
    buffers.give(padded);
    output
}

#[target_feature(enable = "avx2,fma")]
unsafe fn convolve_plane(
    input: &[f32],
    channels: usize,
    width: usize,
    rows: usize,
    height: usize,
    stride: usize,
    weights: &[f32],
    oc: usize,
    bias: f32,
    plane: &mut [f32],
) {
    for y in 0..rows {
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
unsafe fn convolve_four_wide(
    input: &[f32],
    channels: usize,
    width: usize,
    rows: usize,
    height: usize,
    stride: usize,
    weights: &[f32],
    oc: usize,
    bias: &[f32],
    planes: &mut [f32],
    pool_two: bool,
) {
    for y in 0..rows {
        for x in (0..width).step_by(16) {
            let mut a00 = _mm256_setzero_ps();
            let mut a01 = _mm256_setzero_ps();
            let mut a10 = _mm256_setzero_ps();
            let mut a11 = _mm256_setzero_ps();
            let mut a20 = _mm256_setzero_ps();
            let mut a21 = _mm256_setzero_ps();
            let mut a30 = _mm256_setzero_ps();
            let mut a31 = _mm256_setzero_ps();

            for channel in 0..channels {
                for ky in 0..5 {
                    for kx in 0..5 {
                        let offset = (channel * height + y + ky) * stride + x + kx;
                        let k = (channel * 5 + ky) * 5 + kx;
                        // SAFETY: rows are padded to sixteen pixels and four complete channel weights are present.
                        let p0 = unsafe { _mm256_loadu_ps(input.as_ptr().add(offset)) };
                        let p1 = unsafe { _mm256_loadu_ps(input.as_ptr().add(offset + 8)) };
                        let w = _mm256_set1_ps(unsafe {
                            *weights.get_unchecked((oc + 0) * channels * 25 + k)
                        });
                        a00 = _mm256_fmadd_ps(p0, w, a00);
                        a01 = _mm256_fmadd_ps(p1, w, a01);
                        let w = _mm256_set1_ps(unsafe {
                            *weights.get_unchecked((oc + 1) * channels * 25 + k)
                        });
                        a10 = _mm256_fmadd_ps(p0, w, a10);
                        a11 = _mm256_fmadd_ps(p1, w, a11);
                        let w = _mm256_set1_ps(unsafe {
                            *weights.get_unchecked((oc + 2) * channels * 25 + k)
                        });
                        a20 = _mm256_fmadd_ps(p0, w, a20);
                        a21 = _mm256_fmadd_ps(p1, w, a21);
                        let w = _mm256_set1_ps(unsafe {
                            *weights.get_unchecked((oc + 3) * channels * 25 + k)
                        });
                        a30 = _mm256_fmadd_ps(p0, w, a30);
                        a31 = _mm256_fmadd_ps(p1, w, a31);
                    }
                }
            }
            for (plane, pair) in [[a00, a01], [a10, a11], [a20, a21], [a30, a31]]
                .into_iter()
                .enumerate()
            {
                for (part, sum) in pair.into_iter().enumerate() {
                    let start = x + part * 8;
                    if start >= width {
                        continue;
                    }
                    let value = _mm256_max_ps(
                        _mm256_add_ps(sum, _mm256_set1_ps(bias[plane])),
                        _mm256_setzero_ps(),
                    );
                    if pool_two {
                        let pairs = _mm256_max_ps(value, _mm256_permute_ps::<0xb1>(value));
                        let packed = _mm256_castps256_ps128(_mm256_permutevar8x32_ps(
                            pairs,
                            _mm256_setr_epi32(0, 2, 4, 6, 0, 2, 4, 6),
                        ));
                        let offset =
                            plane * (rows / 2) * (width / 2) + (y / 2) * (width / 2) + start / 2;
                        let count = 8.min(width - start) / 2;
                        let mask = _mm_set_epi32(0, 0, -1, -1);
                        // SAFETY: full groups contain four pooled pixels; the final group contains two.
                        unsafe {
                            let target = planes.as_mut_ptr().add(offset);
                            let previous = if y % 2 == 0 {
                                _mm_setzero_ps()
                            } else if count == 4 {
                                _mm_loadu_ps(target)
                            } else {
                                _mm_maskload_ps(target, mask)
                            };
                            let pooled = _mm_max_ps(packed, previous);
                            if count == 4 {
                                _mm_storeu_ps(target, pooled);
                            } else {
                                _mm_maskstore_ps(target, mask, pooled);
                            }
                        }
                        continue;
                    }
                    let offset = plane * rows * width + y * width + start;
                    let count = 8.min(width - start);
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
}

/// Vector max pooling for the pinned 28-to-14 and 14-to-4 spatial shapes.
#[target_feature(enable = "avx2,fma")]
pub(super) unsafe fn pool(
    input: &[f32],
    channels: usize,
    width: usize,
    height: usize,
    stride: usize,
    out: &mut [f32],
) {
    let next = width / stride;
    assert!(input.len() == channels * height * width && out.len() == channels * next * next);
    if stride == 2 && width == 28 {
        for c in 0..channels {
            for y in 0..14 {
                let first = (c * height + y * 2) * 28;
                let second = first + 28;
                let base = (c * 14 + y) * 14;
                for block in 0..3 {
                    let x = block * 8;
                    // SAFETY: these blocks cover complete eight-pixel spans of two rows.
                    let a = unsafe { _mm256_loadu_ps(input.as_ptr().add(first + x)) };
                    let b = unsafe { _mm256_loadu_ps(input.as_ptr().add(second + x)) };
                    let vertical = _mm256_max_ps(a, b);
                    let pairs = _mm256_max_ps(vertical, _mm256_permute_ps::<0xb1>(vertical));
                    let packed =
                        _mm256_permutevar8x32_ps(pairs, _mm256_setr_epi32(0, 2, 4, 6, 0, 2, 4, 6));
                    unsafe {
                        _mm_storeu_ps(
                            out.as_mut_ptr().add(base + block * 4),
                            _mm256_castps256_ps128(packed),
                        );
                    }
                }
                for x in 12..14 {
                    out[base + x] = input[first + x * 2]
                        .max(input[first + x * 2 + 1])
                        .max(input[second + x * 2])
                        .max(input[second + x * 2 + 1]);
                }
            }
        }
    } else {
        assert!(stride == 3 && width == 14);
        for c in 0..channels {
            for y in 0..4 {
                let mut first = _mm256_set1_ps(f32::NEG_INFINITY);
                let mut tail = _mm_set1_ps(f32::NEG_INFINITY);
                for row in 0..3 {
                    let base = (c * height + y * 3 + row) * 14;
                    first =
                        _mm256_max_ps(first, unsafe { _mm256_loadu_ps(input.as_ptr().add(base)) });
                    tail = _mm_max_ps(tail, unsafe { _mm_loadu_ps(input.as_ptr().add(base + 8)) });
                }
                let mut values = [0.0; 12];
                unsafe {
                    _mm256_storeu_ps(values.as_mut_ptr(), first);
                    _mm_storeu_ps(values.as_mut_ptr().add(8), tail);
                }
                for x in 0..4 {
                    out[(c * 4 + y) * 4 + x] =
                        values[x * 3].max(values[x * 3 + 1]).max(values[x * 3 + 2]);
                }
            }
        }
    }
}

/// Ten float32 logits from 256 features, with two independent vector FMA chains.
#[target_feature(enable = "avx2,fma")]
pub(super) unsafe fn head(features: &[f32], weights: &[f32]) -> Vec<f32> {
    assert!(features.len() == 256 && weights.len() == 256 * 10);
    let mut first = _mm256_setzero_ps();
    let mut tail = _mm_setzero_ps();
    let mask = _mm_set_epi32(0, 0, -1, -1);
    for i in 0..256 {
        // SAFETY: eight full weights and two masked weights fit each dense row.
        let w = unsafe { _mm256_loadu_ps(weights.as_ptr().add(i * 10)) };
        let w_tail = unsafe { _mm_maskload_ps(weights.as_ptr().add(i * 10 + 8), mask) };
        first = _mm256_fmadd_ps(_mm256_set1_ps(features[i]), w, first);
        tail = _mm_fmadd_ps(_mm_set1_ps(features[i]), w_tail, tail);
    }
    let mut out = vec![0.0; 10];
    unsafe {
        _mm256_storeu_ps(out.as_mut_ptr(), first);
        _mm_maskstore_ps(out.as_mut_ptr().add(8), mask, tail);
    }
    out
}
