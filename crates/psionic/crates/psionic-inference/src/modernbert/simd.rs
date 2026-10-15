//! Packed float32 AVX2 projections. Weights are packed once; rows stay in f32.
use super::Buffers;
use rayon::prelude::*;
use std::arch::x86_64::*;

pub(super) struct Packed {
    values: Vec<f32>,
    long_values: Vec<f32>,
    inner: usize,
    cols: usize,
}
pub(super) fn available() -> bool {
    is_x86_feature_detected!("avx2") && is_x86_feature_detected!("fma")
}
impl Packed {
    pub(super) fn byte_len(&self) -> usize {
        (self.values.len() + self.long_values.len()) * 4
    }
    pub(super) fn new(values: &[f32], inner: usize, cols: usize) -> Self {
        assert!(inner > 0 && cols > 0);
        assert_eq!(values.len(), inner * cols);
        let mut packed = vec![0.0; cols.div_ceil(24) * inner * 24];
        for tile in 0..cols.div_ceil(24) {
            for k in 0..inner {
                let count = 24.min(cols - tile * 24);
                packed[(tile * inner + k) * 24..(tile * inner + k) * 24 + count]
                    .copy_from_slice(&values[k * cols + tile * 24..k * cols + tile * 24 + count]);
            }
        }
        let mut long_values = vec![0.0; cols.div_ceil(6) * inner * 6];
        for tile in 0..cols.div_ceil(6) {
            for k in 0..inner {
                let count = 6.min(cols - tile * 6);
                long_values[(tile * inner + k) * 6..(tile * inner + k) * 6 + count]
                    .copy_from_slice(&values[k * cols + tile * 6..k * cols + tile * 6 + count]);
            }
        }
        Self {
            long_values,
            values: packed,
            inner,
            cols,
        }
    }
    pub(super) fn multiply(&self, input: &[f32], buffers: &Buffers) -> Vec<f32> {
        assert!(available());
        assert!(!input.is_empty() && input.len() % self.inner == 0);
        let rows = input.len() / self.inner;
        if rows > 64 {
            return self.multiply_long(input, rows, buffers);
        }
        let padded_rows = rows.div_ceil(3) * 3;
        let mut padded = Vec::new();
        let input = if padded_rows == rows {
            input
        } else {
            padded = {
                let mut v = buffers.take(padded_rows * self.inner);
                v[input.len()..].fill(0.0);
                v[..input.len()].copy_from_slice(input);
                v
            };
            &padded
        };
        let mut tiles = buffers.take(self.cols.div_ceil(24) * padded_rows * 24);
        tiles
            .par_chunks_mut(padded_rows * 24)
            .enumerate()
            .for_each(|(tile, out)| {
                // SAFETY: the caller checks AVX2/FMA. Packing pads columns to 24,
                // and the activation rows and output rows are padded to multiples of three.
                unsafe {
                    kernel(
                        input,
                        &self.values[tile * self.inner * 24..(tile + 1) * self.inner * 24],
                        out,
                        padded_rows,
                        self.inner,
                    );
                }
            });
        let mut output = buffers.take(rows * self.cols);
        output
            .par_chunks_mut(self.cols)
            .enumerate()
            .for_each(|(row, out)| {
                for tile in 0..self.cols.div_ceil(24) {
                    let count = 24.min(self.cols - tile * 24);
                    out[tile * 24..tile * 24 + count].copy_from_slice(
                        &tiles[(tile * padded_rows + row) * 24
                            ..(tile * padded_rows + row) * 24 + count],
                    );
                }
            });
        buffers.give(tiles);
        if !padded.is_empty() {
            buffers.give(padded);
        }
        output
    }
    fn multiply_long(&self, input: &[f32], rows: usize, buffers: &Buffers) -> Vec<f32> {
        let padded_rows = rows.div_ceil(16) * 16;
        let mut activations = buffers.take(padded_rows * self.inner);
        activations.fill(0.0);
        activations
            .par_chunks_mut(16 * self.inner)
            .enumerate()
            .for_each(|(block, out)| {
                // SAFETY: activations are padded to sixteen rows and each task owns its block.
                unsafe {
                    pack_rows(input, out, rows, self.inner, block * 16);
                }
            });
        let mut tiles = buffers.take(self.cols.div_ceil(6) * 6 * padded_rows);
        // Reuse a small activation panel across neighboring weight panels. A full
        // long-sequence MLP input can exceed a core's L2 cache.
        let grouped = rows * self.inner > 256 * 1024;
        let group_tiles = if grouped { 4 } else { 1 };
        let block_rows = if grouped { 64 } else { padded_rows };
        tiles
            .par_chunks_mut(6 * padded_rows * group_tiles)
            .enumerate()
            .for_each(|(group, out)| {
                for first in (0..padded_rows).step_by(block_rows) {
                    let count = block_rows.min(padded_rows - first);
                    for (local, tile_out) in out.chunks_mut(6 * padded_rows).enumerate() {
                        let tile = group * group_tiles + local;
                        // SAFETY: every row panel is a multiple of sixteen. Each task
                        // owns complete output columns; the stride remains the full height.
                        unsafe {
                            kernel_long(
                                &activations[first * self.inner..],
                                &self.long_values
                                    [tile * self.inner * 6..(tile + 1) * self.inner * 6],
                                &mut tile_out[first..],
                                count,
                                self.inner,
                                padded_rows,
                            );
                        }
                    }
                }
            });
        let mut output = buffers.take(rows * self.cols);
        output
            .par_chunks_mut(16 * self.cols)
            .enumerate()
            .for_each(|(block, out)| {
                // SAFETY: each task owns complete output rows and the column scratch covers every row.
                unsafe {
                    unpack_columns(&tiles, out, padded_rows, self.cols, block * 16);
                }
            });
        buffers.give(activations);
        buffers.give(tiles);
        output
    }
}

#[target_feature(enable = "avx2,fma")]
unsafe fn kernel(input: &[f32], weight: &[f32], out: &mut [f32], rows: usize, k: usize) {
    for row in (0..rows).step_by(3) {
        let mut a00 = _mm256_setzero_ps();
        let mut a01 = a00;
        let mut a02 = a00;
        let mut a10 = a00;
        let mut a11 = a00;
        let mut a12 = a00;
        let mut a20 = a00;
        let mut a21 = a00;
        let mut a22 = a00;
        for depth in 0..k {
            // SAFETY: this tile contains k rows of 24 weights.
            let w0 = unsafe { _mm256_loadu_ps(weight.as_ptr().add(depth * 24)) };
            let w1 = unsafe { _mm256_loadu_ps(weight.as_ptr().add(depth * 24 + 8)) };
            let w2 = unsafe { _mm256_loadu_ps(weight.as_ptr().add(depth * 24 + 16)) };
            let x = _mm256_set1_ps(unsafe { *input.get_unchecked(row * k + depth) });
            a00 = _mm256_fmadd_ps(x, w0, a00);
            a01 = _mm256_fmadd_ps(x, w1, a01);
            a02 = _mm256_fmadd_ps(x, w2, a02);
            let x = _mm256_set1_ps(unsafe { *input.get_unchecked((row + 1) * k + depth) });
            a10 = _mm256_fmadd_ps(x, w0, a10);
            a11 = _mm256_fmadd_ps(x, w1, a11);
            a12 = _mm256_fmadd_ps(x, w2, a12);
            let x = _mm256_set1_ps(unsafe { *input.get_unchecked((row + 2) * k + depth) });
            a20 = _mm256_fmadd_ps(x, w0, a20);
            a21 = _mm256_fmadd_ps(x, w1, a21);
            a22 = _mm256_fmadd_ps(x, w2, a22);
        }
        for (i, v) in [a00, a01, a02, a10, a11, a12, a20, a21, a22]
            .into_iter()
            .enumerate()
        {
            // SAFETY: the output has three padded rows, each containing 24 f32 elements.
            unsafe {
                _mm256_storeu_ps(out.as_mut_ptr().add(row * 24 + i * 8), v);
            }
        }
    }
}

// Degree-10 least-squares fits of erf on unit intervals, centered at each midpoint.
// Regenerate with scripts/dev/psionic-erf-coefficients.py; dense-domain tests bound the error.
const ERF_COEFFICIENTS: [[f32; 11]; 4] = [
    [
        0.5204998850822449,
        0.8787825703620911,
        -0.43939128518104553,
        -0.14646406471729279,
        0.18307946622371674,
        0.007330315187573433,
        -0.050037458539009094,
        0.005335134919732809,
        0.010012077167630196,
        -0.001864258898422122,
        -0.0014348847325891256,
    ],
    [
        0.9661051630973816,
        0.11893028765916824,
        -0.17839543521404266,
        0.13875216245651245,
        -0.04459894821047783,
        -0.014870165847241879,
        0.019327789545059204,
        -0.004705170169472694,
        -0.0023760413751006126,
        0.001548216911032796,
        -4.158954106969759e-05,
    ],
    [
        0.9995930194854736,
        0.002178284339606762,
        -0.005445710849016905,
        0.008350084535777569,
        -0.008622356690466404,
        0.006117475219070911,
        -0.0027988185174763203,
        0.0005412236205302179,
        0.0002633473777677864,
        -0.00024568039225414395,
        6.877841224195436e-05,
    ],
    [
        0.9999992847442627,
        5.399428573582554e-06,
        -1.8897981135523878e-05,
        4.2295352614019066e-05,
        -6.771847256459296e-05,
        8.212029933929443e-05,
        -7.773233664920554e-05,
        5.815037002321333e-05,
        -3.43548454111442e-05,
        1.5537798390141688e-05,
        -4.2934671000693925e-06,
    ],
];

/// Vector erf-form GeGLU. The caller checks CPU features and complete row extents.
#[target_feature(enable = "avx2,fma")]
pub(super) unsafe fn geglu_row(input: &[f32], out: &mut [f32]) {
    assert_eq!(input.len(), 2 * out.len());
    let f = out.len();
    let blocks = f / 8;
    for block in 0..blocks {
        let i = block * 8;
        // SAFETY: each block has eight elements in both halves and in the output.
        let x = unsafe { _mm256_loadu_ps(input.as_ptr().add(i)) };
        let gate = unsafe { _mm256_loadu_ps(input.as_ptr().add(f + i)) };
        let scaled = _mm256_div_ps(x, _mm256_set1_ps(std::f32::consts::SQRT_2));
        let sign = _mm256_and_ps(scaled, _mm256_set1_ps(-0.0));
        let abs = _mm256_min_ps(
            _mm256_andnot_ps(_mm256_set1_ps(-0.0), scaled),
            _mm256_set1_ps(4.0),
        );
        let mut value = _mm256_set1_ps(1.0);
        for interval in (0..4).rev() {
            let t = _mm256_sub_ps(abs, _mm256_set1_ps(interval as f32 + 0.5));
            let coefficients = &ERF_COEFFICIENTS[interval];
            let mut polynomial = _mm256_set1_ps(coefficients[10]);
            for c in (0..10).rev() {
                polynomial = _mm256_fmadd_ps(polynomial, t, _mm256_set1_ps(coefficients[c]));
            }
            let mask = _mm256_cmp_ps::<_CMP_LT_OQ>(abs, _mm256_set1_ps((interval + 1) as f32));
            value = _mm256_blendv_ps(value, polynomial, mask);
        }
        value = _mm256_xor_ps(value, sign);
        value = _mm256_mul_ps(
            _mm256_mul_ps(
                _mm256_mul_ps(x, _mm256_add_ps(_mm256_set1_ps(1.0), value)),
                _mm256_set1_ps(0.5),
            ),
            gate,
        );
        unsafe {
            _mm256_storeu_ps(out.as_mut_ptr().add(i), value);
        }
    }
    for i in blocks * 8..f {
        out[i] =
            input[i] * (1.0 + libm::erff(input[i] / std::f32::consts::SQRT_2)) * 0.5 * input[f + i];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packed_projection_matches_independent_dot_products_with_tails() {
        if !available() {
            return;
        }
        for (rows, k, cols) in [
            (1, 3, 1),
            (3, 17, 23),
            (4, 7, 24),
            (7, 17, 25),
            (65, 33, 49),
            (129, 64, 48),
            (257, 1152, 49),
        ] {
            let input: Vec<_> = (0..rows * k)
                .map(|i| ((i * 17 % 101) as f32 - 50.0) / 53.0)
                .collect();
            let weights: Vec<_> = (0..k * cols)
                .map(|i| ((i * 13 % 71) as f32 - 35.0) / 79.0)
                .collect();
            let actual = Packed::new(&weights, k, cols).multiply(&input, &Buffers::default());
            for row in 0..rows {
                for col in 0..cols {
                    let expected: f64 = (0..k)
                        .map(|i| input[row * k + i] as f64 * weights[i * cols + col] as f64)
                        .sum();
                    // Longer float32 sums accumulate more rounding error than the f64 oracle.
                    let tolerance = if k > 256 { 1e-4 } else { 1e-5 };
                    assert!((actual[row * cols + col] as f64 - expected).abs() < tolerance);
                }
            }
        }
    }
    #[test]
    fn vector_softmax_matches_scalar_with_masks_and_large_score_ranges() {
        if !available() {
            return;
        }
        for n in [1, 7, 8, 9, 129, 512] {
            for scale in [0.01, 1.0, 100.0] {
                let mut row: Vec<_> = (0..n)
                    .map(|i| {
                        if i % 7 == 6 {
                            f32::NEG_INFINITY
                        } else {
                            ((i * 13 % 71) as f32 - 35.0) * scale
                        }
                    })
                    .collect();
                let maximum = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
                let mut expected: Vec<_> = row.iter().map(|v| (v - maximum).exp()).collect();
                let sum: f32 = expected.iter().sum();
                for v in &mut expected {
                    *v /= sum;
                }
                unsafe {
                    softmax_row(&mut row, maximum);
                }
                assert!(row.iter().zip(&expected).all(|(a, b)| (a - b).abs() < 1e-6));
                assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-5);
            }
        }
    }
    #[test]
    fn vector_norm_matches_float64_reference_with_tails_and_constant_rows() {
        if !available() {
            return;
        }
        for n in [1, 3, 8, 17, 64, 384] {
            for constant in [true, false] {
                let mut row: Vec<_> = (0..n)
                    .map(|i| {
                        if constant {
                            2.0
                        } else {
                            ((i * 13 % 31) as f32 - 15.0) / 17.0
                        }
                    })
                    .collect();
                let weight: Vec<_> = (0..n).map(|i| 1.0 + i as f32 / (n + 1) as f32).collect();
                let mean = row.iter().map(|v| *v as f64).sum::<f64>() / n as f64;
                let variance =
                    row.iter().map(|v| (*v as f64 - mean).powi(2)).sum::<f64>() / n as f64;
                let expected: Vec<_> = row
                    .iter()
                    .zip(&weight)
                    .map(|(v, w)| ((*v as f64 - mean) / (variance + 1e-5).sqrt()) * (*w as f64))
                    .collect();
                unsafe {
                    normalize_row(&mut row, &weight, 1e-5);
                }
                assert!(
                    row.iter()
                        .zip(&expected)
                        .all(|(a, b)| (*a as f64 - b).abs() < 3e-6)
                );
            }
        }
    }
    #[test]
    fn vector_erf_form_matches_libm_over_dense_domain_and_boundaries() {
        if !available() {
            return;
        }
        let mut input = vec![0.0; 2 * 800008];
        let f = input.len() / 2;
        for (i, x) in input[..f].iter_mut().enumerate() {
            *x = (i as f32 / (f - 1) as f32) * 24.0 - 12.0;
        }
        for x in &mut input[f..] {
            *x = 1.0;
        }
        let mut actual = vec![0.0; f];
        unsafe {
            geglu_row(&input, &mut actual);
        }
        let mut distance = 0.0_f32;
        for i in 0..f {
            let expected = input[i] * (1.0 + libm::erff(input[i] / std::f32::consts::SQRT_2)) * 0.5;
            distance = distance.max((actual[i] - expected).abs());
        }
        assert!(distance < 1e-6, "GeGLU max error={distance}");
    }
}

/// Float32 softmax with a range-reduced degree-seven vector exponential.
/// Inputs are already masked; values below the normal f32 range become zero.
#[target_feature(enable = "avx2,fma")]
pub(super) unsafe fn softmax_row(row: &mut [f32], maximum: f32) {
    let mut sums = _mm256_setzero_ps();
    let blocks = row.len() / 8;
    for block in 0..blocks {
        let i = block * 8;
        let input = unsafe { _mm256_loadu_ps(row.as_ptr().add(i)) };
        let x = _mm256_sub_ps(input, _mm256_set1_ps(maximum));
        let normal = _mm256_cmp_ps::<_CMP_GE_OQ>(x, _mm256_set1_ps(-87.0));
        let x = _mm256_max_ps(x, _mm256_set1_ps(-87.0));
        let n = _mm256_round_ps::<{ _MM_FROUND_TO_NEAREST_INT | _MM_FROUND_NO_EXC }>(
            _mm256_mul_ps(x, _mm256_set1_ps(std::f32::consts::LOG2_E)),
        );
        let r = _mm256_fnmadd_ps(n, _mm256_set1_ps(0.69314575), x);
        let r = _mm256_fnmadd_ps(n, _mm256_set1_ps(1.4286068e-6), r);
        let mut p = _mm256_set1_ps(1.0 / 5040.0);
        for coefficient in [
            1.0 / 720.0,
            1.0 / 120.0,
            1.0 / 24.0,
            1.0 / 6.0,
            0.5,
            1.0,
            1.0,
        ] {
            p = _mm256_fmadd_ps(p, r, _mm256_set1_ps(coefficient));
        }
        let bits = _mm256_slli_epi32::<23>(_mm256_add_epi32(
            _mm256_cvtps_epi32(n),
            _mm256_set1_epi32(127),
        ));
        let values = _mm256_and_ps(normal, _mm256_mul_ps(p, _mm256_castsi256_ps(bits)));
        sums = _mm256_add_ps(sums, values);
        unsafe {
            _mm256_storeu_ps(row.as_mut_ptr().add(i), values);
        }
    }
    let mut lanes = [0.0; 8];
    unsafe {
        _mm256_storeu_ps(lanes.as_mut_ptr(), sums);
    }
    let mut sum: f32 = lanes.iter().sum();
    for value in &mut row[blocks * 8..] {
        *value = (*value - maximum).exp();
        sum += *value;
    }
    let inv = 1.0 / sum;
    for block in 0..blocks {
        let i = block * 8;
        let values = unsafe { _mm256_loadu_ps(row.as_ptr().add(i)) };
        unsafe {
            _mm256_storeu_ps(
                row.as_mut_ptr().add(i),
                _mm256_mul_ps(values, _mm256_set1_ps(inv)),
            );
        }
    }
    for value in &mut row[blocks * 8..] {
        *value *= inv;
    }
}

/// Split-half rotary embedding with cached position frequencies, in float32.
#[target_feature(enable = "avx2,fma")]
pub(super) unsafe fn rotate(values: &mut [f32], dim: usize, rotary: &[f32], scale: f32) {
    assert!(dim > 0 && dim % 2 == 0 && values.len() % dim == 0 && rotary.len() >= values.len());
    let half = dim / 2;
    for (t, row) in values.chunks_mut(dim).enumerate() {
        for block in 0..half / 8 {
            let i = block * 8;
            // SAFETY: complete vectors fit both halves of the row and its frequency table.
            let a = unsafe { _mm256_loadu_ps(row.as_ptr().add(i)) };
            let b = unsafe { _mm256_loadu_ps(row.as_ptr().add(half + i)) };
            let sin = unsafe { _mm256_loadu_ps(rotary.as_ptr().add(t * dim + i)) };
            let cos = unsafe { _mm256_loadu_ps(rotary.as_ptr().add(t * dim + half + i)) };
            let first = _mm256_mul_ps(
                _mm256_sub_ps(_mm256_mul_ps(a, cos), _mm256_mul_ps(b, sin)),
                _mm256_set1_ps(scale),
            );
            let second = _mm256_mul_ps(
                _mm256_add_ps(_mm256_mul_ps(b, cos), _mm256_mul_ps(a, sin)),
                _mm256_set1_ps(scale),
            );
            unsafe {
                _mm256_storeu_ps(row.as_mut_ptr().add(i), first);
                _mm256_storeu_ps(row.as_mut_ptr().add(half + i), second);
            }
        }
        for d in half / 8 * 8..half {
            let sin = rotary[t * dim + d];
            let cos = rotary[t * dim + half + d];
            let a = row[d];
            let b = row[half + d];
            row[d] = (a * cos - b * sin) * scale;
            row[half + d] = (b * cos + a * sin) * scale;
        }
    }
}

#[target_feature(enable = "avx2,fma")]
unsafe fn kernel_long(
    input: &[f32],
    weights: &[f32],
    out: &mut [f32],
    rows: usize,
    k: usize,
    stride: usize,
) {
    for block in 0..rows / 16 {
        let mut a00 = _mm256_setzero_ps();
        let mut a01 = _mm256_setzero_ps();
        let mut a10 = _mm256_setzero_ps();
        let mut a11 = _mm256_setzero_ps();
        let mut a20 = _mm256_setzero_ps();
        let mut a21 = _mm256_setzero_ps();
        let mut a30 = _mm256_setzero_ps();
        let mut a31 = _mm256_setzero_ps();
        let mut a40 = _mm256_setzero_ps();
        let mut a41 = _mm256_setzero_ps();
        let mut a50 = _mm256_setzero_ps();
        let mut a51 = _mm256_setzero_ps();

        for depth in 0..k {
            // SAFETY: the packed input and weight extents were checked before dispatch.
            let x0 = unsafe { _mm256_loadu_ps(input.as_ptr().add((block * k + depth) * 16)) };
            let x1 = unsafe { _mm256_loadu_ps(input.as_ptr().add((block * k + depth) * 16 + 8)) };
            let w = _mm256_set1_ps(unsafe { *weights.get_unchecked(depth * 6 + 0) });
            a00 = _mm256_fmadd_ps(x0, w, a00);
            a01 = _mm256_fmadd_ps(x1, w, a01);
            let w = _mm256_set1_ps(unsafe { *weights.get_unchecked(depth * 6 + 1) });
            a10 = _mm256_fmadd_ps(x0, w, a10);
            a11 = _mm256_fmadd_ps(x1, w, a11);
            let w = _mm256_set1_ps(unsafe { *weights.get_unchecked(depth * 6 + 2) });
            a20 = _mm256_fmadd_ps(x0, w, a20);
            a21 = _mm256_fmadd_ps(x1, w, a21);
            let w = _mm256_set1_ps(unsafe { *weights.get_unchecked(depth * 6 + 3) });
            a30 = _mm256_fmadd_ps(x0, w, a30);
            a31 = _mm256_fmadd_ps(x1, w, a31);
            let w = _mm256_set1_ps(unsafe { *weights.get_unchecked(depth * 6 + 4) });
            a40 = _mm256_fmadd_ps(x0, w, a40);
            a41 = _mm256_fmadd_ps(x1, w, a41);
            let w = _mm256_set1_ps(unsafe { *weights.get_unchecked(depth * 6 + 5) });
            a50 = _mm256_fmadd_ps(x0, w, a50);
            a51 = _mm256_fmadd_ps(x1, w, a51);
        }
        unsafe {
            _mm256_storeu_ps(out.as_mut_ptr().add(0 * stride + block * 16), a00);
            _mm256_storeu_ps(out.as_mut_ptr().add(0 * stride + block * 16 + 8), a01);
        }
        unsafe {
            _mm256_storeu_ps(out.as_mut_ptr().add(1 * stride + block * 16), a10);
            _mm256_storeu_ps(out.as_mut_ptr().add(1 * stride + block * 16 + 8), a11);
        }
        unsafe {
            _mm256_storeu_ps(out.as_mut_ptr().add(2 * stride + block * 16), a20);
            _mm256_storeu_ps(out.as_mut_ptr().add(2 * stride + block * 16 + 8), a21);
        }
        unsafe {
            _mm256_storeu_ps(out.as_mut_ptr().add(3 * stride + block * 16), a30);
            _mm256_storeu_ps(out.as_mut_ptr().add(3 * stride + block * 16 + 8), a31);
        }
        unsafe {
            _mm256_storeu_ps(out.as_mut_ptr().add(4 * stride + block * 16), a40);
            _mm256_storeu_ps(out.as_mut_ptr().add(4 * stride + block * 16 + 8), a41);
        }
        unsafe {
            _mm256_storeu_ps(out.as_mut_ptr().add(5 * stride + block * 16), a50);
            _mm256_storeu_ps(out.as_mut_ptr().add(5 * stride + block * 16 + 8), a51);
        }
    }
}

/// Transposes full four-by-four blocks from column-major scratch to row-major output.
#[target_feature(enable = "avx2,fma")]
unsafe fn unpack_columns(input: &[f32], out: &mut [f32], rows: usize, cols: usize, first: usize) {
    let count = out.len() / cols;
    assert!(input.len() >= rows * cols && out.len() == count * cols && first + count <= rows);
    for row in (0..count / 4 * 4).step_by(4) {
        for col in (0..cols / 4 * 4).step_by(4) {
            // SAFETY: each block fits four complete rows and columns.
            let mut a = unsafe { _mm_loadu_ps(input.as_ptr().add(col * rows + first + row)) };
            let mut b = unsafe { _mm_loadu_ps(input.as_ptr().add((col + 1) * rows + first + row)) };
            let mut c = unsafe { _mm_loadu_ps(input.as_ptr().add((col + 2) * rows + first + row)) };
            let mut d = unsafe { _mm_loadu_ps(input.as_ptr().add((col + 3) * rows + first + row)) };
            _MM_TRANSPOSE4_PS(&mut a, &mut b, &mut c, &mut d);
            unsafe {
                _mm_storeu_ps(out.as_mut_ptr().add(row * cols + col), a);
                _mm_storeu_ps(out.as_mut_ptr().add((row + 1) * cols + col), b);
                _mm_storeu_ps(out.as_mut_ptr().add((row + 2) * cols + col), c);
                _mm_storeu_ps(out.as_mut_ptr().add((row + 3) * cols + col), d);
            }
        }
    }
    for row in 0..count {
        for col in cols / 4 * 4..cols {
            out[row * cols + col] = input[col * rows + first + row];
        }
    }
    for row in count / 4 * 4..count {
        for col in 0..cols / 4 * 4 {
            out[row * cols + col] = input[col * rows + first + row];
        }
    }
}

/// Packs sixteen activation rows with a four-by-four vector transpose.
#[target_feature(enable = "avx2,fma")]
unsafe fn pack_rows(input: &[f32], out: &mut [f32], rows: usize, k: usize, first: usize) {
    let count = 16.min(rows - first);
    assert!(input.len() == rows * k && out.len() == 16 * k);
    for row in (0..count / 4 * 4).step_by(4) {
        for col in (0..k / 4 * 4).step_by(4) {
            let mut a = unsafe { _mm_loadu_ps(input.as_ptr().add((first + row) * k + col)) };
            let mut b = unsafe { _mm_loadu_ps(input.as_ptr().add((first + row + 1) * k + col)) };
            let mut c = unsafe { _mm_loadu_ps(input.as_ptr().add((first + row + 2) * k + col)) };
            let mut d = unsafe { _mm_loadu_ps(input.as_ptr().add((first + row + 3) * k + col)) };
            _MM_TRANSPOSE4_PS(&mut a, &mut b, &mut c, &mut d);
            unsafe {
                _mm_storeu_ps(out.as_mut_ptr().add(col * 16 + row), a);
                _mm_storeu_ps(out.as_mut_ptr().add((col + 1) * 16 + row), b);
                _mm_storeu_ps(out.as_mut_ptr().add((col + 2) * 16 + row), c);
                _mm_storeu_ps(out.as_mut_ptr().add((col + 3) * 16 + row), d);
            }
        }
    }
    for row in 0..count {
        for col in k / 4 * 4..k {
            out[col * 16 + row] = input[(first + row) * k + col];
        }
    }
    for row in count / 4 * 4..count {
        for col in 0..k / 4 * 4 {
            out[col * 16 + row] = input[(first + row) * k + col];
        }
    }
}

/// Two-pass bias-free LayerNorm with float32 vector reductions.
#[target_feature(enable = "avx2,fma")]
pub(super) unsafe fn normalize_row(row: &mut [f32], weight: &[f32], epsilon: f32) {
    assert!(!row.is_empty() && row.len() == weight.len());
    let blocks = row.len() / 8;
    let mut sum = _mm256_setzero_ps();
    for i in 0..blocks {
        let x = unsafe { _mm256_loadu_ps(row.as_ptr().add(i * 8)) };
        sum = _mm256_add_ps(sum, x);
    }
    let mut lanes = [0.0; 8];
    unsafe {
        _mm256_storeu_ps(lanes.as_mut_ptr(), sum);
    }
    let mean =
        (lanes.iter().sum::<f32>() + row[blocks * 8..].iter().sum::<f32>()) / row.len() as f32;
    let mut variance = _mm256_setzero_ps();
    for i in 0..blocks {
        let x = unsafe { _mm256_loadu_ps(row.as_ptr().add(i * 8)) };
        let centered = _mm256_sub_ps(x, _mm256_set1_ps(mean));
        variance = _mm256_fmadd_ps(centered, centered, variance);
    }
    unsafe {
        _mm256_storeu_ps(lanes.as_mut_ptr(), variance);
    }
    let variance = (lanes.iter().sum::<f32>()
        + row[blocks * 8..]
            .iter()
            .map(|x| (x - mean) * (x - mean))
            .sum::<f32>())
        / row.len() as f32;
    let scale = 1.0 / (variance + epsilon).sqrt();
    for i in 0..blocks {
        let offset = i * 8;
        let x = unsafe { _mm256_loadu_ps(row.as_ptr().add(offset)) };
        let w = unsafe { _mm256_loadu_ps(weight.as_ptr().add(offset)) };
        let output = _mm256_mul_ps(
            _mm256_mul_ps(
                _mm256_sub_ps(x, _mm256_set1_ps(mean)),
                _mm256_set1_ps(scale),
            ),
            w,
        );
        unsafe {
            _mm256_storeu_ps(row.as_mut_ptr().add(offset), output);
        }
    }
    for i in blocks * 8..row.len() {
        row[i] = (row[i] - mean) * scale * weight[i];
    }
}
