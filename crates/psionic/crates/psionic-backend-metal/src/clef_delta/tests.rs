use super::*;

#[derive(Clone)]
pub(crate) struct Case {
    pub n: usize,
    pub kh: usize,
    pub vh: usize,
    pub reordered: bool,
    pub width: usize,
    pub offset: usize,
    pub q: Vec<f32>,
    pub k: Vec<f32>,
    pub conv: Vec<f32>,
    pub decay: Vec<f32>,
    pub beta: Vec<f32>,
    pub state: Vec<f32>,
}

impl Case {
    pub fn new(n: usize, kh: usize, vh: usize, reordered: bool) -> Self {
        let offset = 16;
        let width = offset + vh * 128 + 16;
        let values = |len: usize, seed: usize, denominator: f32| {
            (0..len)
                .map(|i| (((i * 13 + i / 7 + seed) % 17) as f32 - 8.0) / denominator)
                .collect::<Vec<_>>()
        };
        Self {
            n,
            kh,
            vh,
            reordered,
            width,
            offset,
            q: values(n * kh * 128, 3, 1024.0),
            k: values(n * kh * 128, 11, 64.0),
            conv: values(n * width, 5, 16.0),
            decay: (0..n * vh)
                .map(|i| match i % 11 {
                    0 => 0.0,
                    1 => 1.0,
                    2 => 2.0f32.powi(-40),
                    _ => 0.875,
                })
                .collect(),
            beta: (0..n * vh).map(|i| [0.0, 0.25, 0.5, 1.0][i % 4]).collect(),
            state: values(vh * 128 * 128, 7, 512.0),
        }
    }

    pub fn key_head(&self, value_head: usize) -> usize {
        if self.reordered {
            value_head % self.kh
        } else {
            value_head / (self.vh / self.kh)
        }
    }

    /// Nearly parallel normalized keys and persistent state stress cancellation
    /// in the triangular solve without resetting the recurrence between blocks.
    pub fn correlated(n: usize, kh: usize, vh: usize, reordered: bool) -> Self {
        let mut case = Self::new(n, kh, vh, reordered);
        for t in 0..n {
            for h in 0..kh {
                let key: Vec<f64> = (0..128)
                    .map(|i| {
                        ((i * 7 + h * 11) % 17) as f64 / 16.0 - 0.5
                            + ((t * 3 + i * 5) % 11) as f64 / 4096.0
                    })
                    .collect();
                let norm = key.iter().map(|x| x * x).sum::<f64>().sqrt();
                for i in 0..128 {
                    case.k[(t * kh + h) * 128 + i] = (key[i] / norm) as f32;
                    case.q[(t * kh + h) * 128 + i] =
                        (key[(i + 7) % 128] / norm / 128.0f64.sqrt()) as f32;
                }
            }
            for h in 0..vh {
                case.decay[t * vh + h] = if (t + h) % 7 == 0 {
                    1.0
                } else {
                    1023.0 / 1024.0
                };
                case.beta[t * vh + h] = [0.75, 0.9375, 1.0][(t + h) % 3];
                for v in 0..128 {
                    case.conv[t * case.width + case.offset + h * 128 + v] =
                        ((t * 37 + h * 17 + v * 13) % 97) as f32 / 16.0 - 3.0;
                }
            }
        }
        for value in &mut case.state {
            *value *= 16.0;
        }
        case
    }
}

/// Independent dense recurrence: update every state element, then apply Q.
pub(crate) fn serial(case: &Case) -> (Vec<f64>, Vec<f64>) {
    let mut state: Vec<f64> = case.state.iter().map(|&x| f64::from(x)).collect();
    let mut output = vec![0.0; case.n * case.vh * 128];
    for t in 0..case.n {
        for h in 0..case.vh {
            let qk = (t * case.kh + case.key_head(h)) * 128;
            let g = f64::from(case.decay[t * case.vh + h]);
            let beta = f64::from(case.beta[t * case.vh + h]);
            for v in 0..128 {
                let s = &mut state[(h * 128 + v) * 128..(h * 128 + v + 1) * 128];
                for value in s.iter_mut() {
                    *value *= g;
                }
                let predicted: f64 = s
                    .iter()
                    .zip(&case.k[qk..qk + 128])
                    .map(|(s, &k)| s * f64::from(k))
                    .sum();
                let delta = (f64::from(case.conv[t * case.width + case.offset + h * 128 + v])
                    - predicted)
                    * beta;
                for (s, &k) in s.iter_mut().zip(&case.k[qk..qk + 128]) {
                    *s += delta * f64::from(k);
                }
                output[(t * case.vh + h) * 128 + v] = s
                    .iter()
                    .zip(&case.q[qk..qk + 128])
                    .map(|(s, &q)| s * f64::from(q))
                    .sum();
            }
        }
    }
    (output, state)
}

// Algebra reference, independent of the serial oracle and GPU implementation.
// Solve each correction from the lower-triangular equations without forming T.
fn blocked(case: &Case, block: usize) -> (Vec<f64>, Vec<f64>) {
    let mut state: Vec<f64> = case.state.iter().map(|&x| f64::from(x)).collect();
    let mut output = vec![0.0; case.n * case.vh * 128];
    for first in (0..case.n).step_by(block) {
        let count = block.min(case.n - first);
        for h in 0..case.vh {
            let key_head = case.key_head(h);
            let qk = |t: usize, k: usize| ((first + t) * case.kh + key_head) * 128 + k;
            let g = |t: usize| f64::from(case.decay[(first + t) * case.vh + h]);
            let interval = |start: usize, end: usize| (start..end).map(g).product::<f64>();
            let cumulative = (0..count).map(|t| interval(0, t + 1)).collect::<Vec<_>>();
            let mut lower = vec![0.0; count * count];
            let mut attention = vec![0.0; count * count];
            for t in 0..count {
                for j in 0..=t {
                    let decay = interval(j + 1, t + 1);
                    lower[t * count + j] = decay
                        * (0..128)
                            .map(|k| f64::from(case.k[qk(t, k)]) * f64::from(case.k[qk(j, k)]))
                            .sum::<f64>();
                    attention[t * count + j] = decay
                        * (0..128)
                            .map(|k| f64::from(case.q[qk(t, k)]) * f64::from(case.k[qk(j, k)]))
                            .sum::<f64>();
                }
            }
            let mut u = vec![0.0; count * 128];
            for t in 0..count {
                let beta = f64::from(case.beta[(first + t) * case.vh + h]);
                for v in 0..128 {
                    let carried: f64 = (0..128)
                        .map(|k| state[(h * 128 + v) * 128 + k] * f64::from(case.k[qk(t, k)]))
                        .sum();
                    let mut value = beta
                        * (f64::from(
                            case.conv[(first + t) * case.width + case.offset + h * 128 + v],
                        ) - cumulative[t] * carried);
                    for j in 0..t {
                        value -= beta * lower[t * count + j] * u[j * 128 + v];
                    }
                    u[t * 128 + v] = value;
                }
                for v in 0..128 {
                    let carried: f64 = (0..128)
                        .map(|k| state[(h * 128 + v) * 128 + k] * f64::from(case.q[qk(t, k)]))
                        .sum();
                    let mut value = cumulative[t] * carried;
                    for j in 0..=t {
                        value += attention[t * count + j] * u[j * 128 + v];
                    }
                    output[((first + t) * case.vh + h) * 128 + v] = value;
                }
            }
            let final_decay = (0..count)
                .map(|j| interval(j + 1, count))
                .collect::<Vec<_>>();
            for v in 0..128 {
                for k in 0..128 {
                    let s = &mut state[(h * 128 + v) * 128 + k];
                    *s *= cumulative[count - 1];
                    for j in 0..count {
                        *s += u[j * 128 + v] * final_decay[j] * f64::from(case.k[qk(j, k)]);
                    }
                }
            }
        }
    }
    (output, state)
}

fn close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len());
    for (i, (a, b)) in actual.iter().zip(expected).enumerate() {
        assert!(
            a.is_finite() && (a - b).abs() < 2e-12,
            "element {i}: {a} vs {b}"
        );
    }
}

#[test]
fn direct_solve_matches_dense_recurrence_with_correlated_keys_and_long_carry() {
    let case = Case::correlated(155, 1, 1, false);
    let expected = serial(&case);
    let actual = blocked(&case, CLEF_DELTA_BLOCK);
    close(&actual.0, &expected.0);
    close(&actual.1, &expected.1);
}

#[test]
fn selection_is_explicit_and_defaults_to_sequential() {
    assert_eq!(ClefMetalDelta::default(), ClefMetalDelta::Sequential);
    assert_eq!("sequential".parse(), Ok(ClefMetalDelta::Sequential));
    assert_eq!(
        "block-tensorops".parse(),
        Ok(ClefMetalDelta::BlockTensorOps)
    );
    for value in ["", "parallel", " block-tensorops", "BLOCK-TENSOROPS"] {
        assert!(value.parse::<ClefMetalDelta>().is_err());
    }
}

#[test]
fn plan_checks_heads_extents_and_value_rows_even_when_empty() {
    for n in [0, 1] {
        for (kh, vh, dim, width, offset) in [
            (0, 4, 128, 512, 0),
            (2, 3, 128, 512, 0),
            (2, 4, 64, 512, 0),
            (2, 4, 128, 511, 0),
            (2, 4, 128, 512, 1),
            (2, 4, 128, usize::MAX, 0),
        ] {
            assert!(ClefDeltaPlan::new(n, kh, vh, dim, false, width, offset).is_err());
        }
    }
    assert!(ClefDeltaPlan::new(usize::MAX, 1, 1, 128, false, 128, 0).is_err());
    assert!(ClefDeltaPlan::new(1, 1, usize::MAX, 128, false, 128, 0).is_err());
    assert!(ClefDeltaPlan::new(1, 1, 1, 128, false, usize::MAX, usize::MAX).is_err());
}

#[test]
fn empty_request_accesses_nothing_and_tails_allocate_whole_blocks() {
    let empty = ClefDeltaPlan::new(0, 2, 6, 128, true, 784, 16).unwrap();
    assert_eq!(
        (empty.blocks, empty.workspace_bytes, empty.state_bytes),
        (0, 0, 0)
    );
    assert_eq!(
        (empty.query_key_bytes, empty.conv_bytes, empty.output_bytes),
        (0, 0, 0)
    );
    assert_eq!(empty.decay_beta_bytes, 0);
    for (n, blocks) in [(1, 1), (31, 1), (32, 1), (33, 2), (65, 3)] {
        let plan = ClefDeltaPlan::new(n, 2, 6, 128, true, 800, 16).unwrap();
        assert_eq!(plan.blocks, blocks);
        assert_eq!(plan.workspace_bytes, blocks * 6 * 37_120);
        assert_eq!(plan.query_key_bytes, n * 2 * 128 * 4);
        assert_eq!(plan.conv_bytes, n * 800 * 4);
        assert_eq!(plan.state_bytes, 6 * 128 * 128 * 4);
        assert_eq!(plan.output_bytes, n * 6 * 128 * 4);
    }
}

#[test]
fn additional_scratch_is_bounded_before_allocation() {
    assert_eq!(
        ClefDeltaPlan::workspace_bytes(1024, 32).unwrap(),
        38_010_880
    );
    assert_eq!(
        ClefDeltaPlan::workspace_bytes(2048, 48).unwrap(),
        114_032_640
    );
    assert!(ClefDeltaPlan::workspace_bytes(16_384, 32).is_err());
    assert!(ClefDeltaPlan::workspace_bytes(usize::MAX, 32).is_err());
    assert!(ClefDeltaPlan::workspace_bytes(0, 0).is_err());
}

#[test]
fn block_algebra_matches_dense_recurrence_and_final_state() {
    for reordered in [false, true] {
        let case = Case::new(33, 2, 4, reordered);
        let expected = serial(&case);
        for block in [1, 16, 32] {
            let actual = blocked(&case, block);
            close(&actual.0, &expected.0);
            close(&actual.1, &expected.1);
        }
    }
}

#[test]
fn zero_beta_keeps_only_decayed_initial_state() {
    let mut case = Case::new(33, 1, 1, false);
    case.beta.fill(0.0);
    case.decay.fill(1.0);
    let result = blocked(&case, 32);
    close(
        &result.1,
        &case.state.iter().map(|&x| f64::from(x)).collect::<Vec<_>>(),
    );
    close(&result.0, &serial(&case).0);
    case.decay[31] = 0.0;
    let result = blocked(&case, 32);
    assert!(result.1.iter().all(|&x| x == 0.0));
    assert!(result.0[31 * 128..].iter().all(|&x| x == 0.0));
}

#[test]
fn head_orders_are_distinct_and_each_matches_its_oracle() {
    let grouped = Case::new(2, 2, 4, false);
    let mut reordered = grouped.clone();
    reordered.reordered = true;
    assert_eq!((grouped.key_head(1), reordered.key_head(1)), (0, 1));
    assert_ne!(serial(&grouped).0, serial(&reordered).0);
    close(&blocked(&reordered, 32).0, &serial(&reordered).0);
}
