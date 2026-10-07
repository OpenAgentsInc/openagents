//! A square two-dimensional inverse FFT in `f32`: iterative radix-2
//! Cooley–Tukey ("An Algorithm for the Machine Calculation of Complex
//! Fourier Series", 1965) with the real and imaginary parts in separate
//! arrays. Each pass transforms every column at once, so the butterfly's
//! inner loop runs along a contiguous row and vectorizes; a transpose
//! between the passes turns rows into columns.
//!
//! The spectral surface needs nothing more: sizes are powers of two, and
//! the transform is unnormalized, `x[j] = Σ X[n] e^{+2πi n j / N}`.

/// Inverse transforms of one power-of-two size.
#[derive(Clone, Debug)]
pub struct Fft2 {
    n: usize,
    /// `e^{+2πi j / n}` for `j < n / 2`.
    cos: Vec<f32>,
    sin: Vec<f32>,
    /// Each index's bit reversal.
    reverse: Vec<usize>,
}

impl Fft2 {
    /// Transforms of `n × n` grids.
    ///
    /// # Panics
    ///
    /// When `n` is not a power of two of at least 2.
    #[must_use]
    pub fn new(n: usize) -> Self {
        assert!(n >= 2 && n.is_power_of_two(), "FFT size {n}");
        let bits = n.trailing_zeros();
        let (cos, sin) = (0..n / 2)
            .map(|j| {
                let a = std::f64::consts::TAU * j as f64 / n as f64;
                (a.cos() as f32, a.sin() as f32)
            })
            .unzip();
        let reverse = (0..n)
            .map(|i| i.reverse_bits() >> (usize::BITS - bits))
            .collect();
        Self {
            n,
            cos,
            sin,
            reverse,
        }
    }

    /// The side of the grids this transforms.
    #[must_use]
    pub fn size(&self) -> usize {
        self.n
    }

    /// Replaces the `n × n` grid (`re`, `im`, row-major) with its inverse
    /// transform.
    ///
    /// # Panics
    ///
    /// When either slice is not `n²` long.
    pub fn inverse(&self, re: &mut [f32], im: &mut [f32]) {
        let n = self.n;
        assert!(re.len() == n * n && im.len() == n * n);
        self.columns(re, im);
        transpose(re, n);
        transpose(im, n);
        self.columns(re, im);
        transpose(re, n);
        transpose(im, n);
    }

    /// Transforms every column: rows are swapped into bit-reversed order,
    /// then each butterfly combines two whole rows.
    fn columns(&self, re: &mut [f32], im: &mut [f32]) {
        let n = self.n;
        for (i, &j) in self.reverse.iter().enumerate() {
            if i < j {
                swap_rows(re, n, i, j);
                swap_rows(im, n, i, j);
            }
        }
        let mut size = 2;
        while size <= n {
            let half = size / 2;
            let step = n / size;
            for start in (0..n).step_by(size) {
                for j in 0..half {
                    let (wr, wi) = (self.cos[j * step], self.sin[j * step]);
                    let a = (start + j) * n;
                    let b = a + half * n;
                    let (re_a, re_b) = re.split_at_mut(b);
                    let (im_a, im_b) = im.split_at_mut(b);
                    butterfly(
                        &mut re_a[a..a + n],
                        &mut im_a[a..a + n],
                        &mut re_b[..n],
                        &mut im_b[..n],
                        wr,
                        wi,
                    );
                }
            }
            size *= 2;
        }
    }
}

/// `a, b ← a + w b, a − w b` along two rows.
#[inline]
fn butterfly(ar: &mut [f32], ai: &mut [f32], br: &mut [f32], bi: &mut [f32], wr: f32, wi: f32) {
    let len = ar.len();
    let (ai, br, bi) = (&mut ai[..len], &mut br[..len], &mut bi[..len]);
    for c in 0..len {
        let tr = wr * br[c] - wi * bi[c];
        let ti = wr * bi[c] + wi * br[c];
        br[c] = ar[c] - tr;
        bi[c] = ai[c] - ti;
        ar[c] += tr;
        ai[c] += ti;
    }
}

fn swap_rows(v: &mut [f32], n: usize, i: usize, j: usize) {
    let (low, high) = v.split_at_mut(j * n);
    low[i * n..i * n + n].swap_with_slice(&mut high[..n]);
}

/// Transposes a square grid in place, in 16 × 16 blocks for the cache.
fn transpose(v: &mut [f32], n: usize) {
    const BLOCK: usize = 16;
    for bi in (0..n).step_by(BLOCK) {
        for bj in (bi..n).step_by(BLOCK) {
            for i in bi..(bi + BLOCK).min(n) {
                let from = if bi == bj { i + 1 } else { bj };
                for j in from..(bj + BLOCK).min(n) {
                    v.swap(i * n + j, j * n + i);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The transform matches the direct sum on a pseudo-random grid.
    #[test]
    fn matches_the_direct_sum() {
        for n in [2usize, 4, 16, 32] {
            let mut state = 0x1234_5678u32;
            let mut next = || {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 8) as f32 / 16_777_216.0 - 0.5
            };
            let re0: Vec<f32> = (0..n * n).map(|_| next()).collect();
            let im0: Vec<f32> = (0..n * n).map(|_| next()).collect();
            let (mut re, mut im) = (re0.clone(), im0.clone());
            Fft2::new(n).inverse(&mut re, &mut im);
            for y in 0..n {
                for x in 0..n {
                    let (mut sr, mut si) = (0.0f64, 0.0f64);
                    for v in 0..n {
                        for u in 0..n {
                            let a = std::f64::consts::TAU * ((u * x + v * y) % n) as f64 / n as f64;
                            let (s, c) = a.sin_cos();
                            let (r, i) = (f64::from(re0[v * n + u]), f64::from(im0[v * n + u]));
                            sr += r * c - i * s;
                            si += r * s + i * c;
                        }
                    }
                    assert!((sr - f64::from(re[y * n + x])).abs() < 1e-3, "{n} {x} {y}");
                    assert!((si - f64::from(im[y * n + x])).abs() < 1e-3, "{n} {x} {y}");
                }
            }
        }
    }
}
