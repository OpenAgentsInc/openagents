//! Explicit hardware checks for the experimental block32 scan.

use super::*;
use crate::clef_delta::tests::{Case, serial};
use crate::clef_delta::{ClefDeltaPlan, ClefMetalDelta};

const GUARD: usize = 16;
const SENTINEL: f32 = 913.25;

struct Guarded {
    allocation: ClefMetalBuffer,
    view: ClefMetalBuffer,
    len: usize,
}

impl Guarded {
    fn new(metal: &ClefMetal, values: &[f32]) -> Self {
        let allocation = metal.buffer_f32(&vec![SENTINEL; values.len() + 2 * GUARD]);
        allocation.write_f32(GUARD, values).unwrap();
        let view = allocation.view(GUARD * 4, values.len() * 4).unwrap();
        Self {
            allocation,
            view,
            len: values.len(),
        }
    }

    fn check(&self) {
        for first in [0, GUARD + self.len] {
            assert!(
                self.allocation
                    .read_f32(first, GUARD)
                    .unwrap()
                    .iter()
                    .all(|x| x.to_bits() == SENTINEL.to_bits())
            );
        }
    }

    fn read(&self) -> Vec<f32> {
        self.check();
        self.view.read_f32(0, self.len).unwrap()
    }
}

fn metal() -> ClefMetal {
    ClefMetal::new_with_execution_options(
        ClefMetalAttention::Staged,
        ClefMetalGemm::Fixed64,
        ClefMetalDelta::BlockTensorOps,
    )
    .expect("explicit block DeltaNet device")
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn delta_prep_matches_additive_epsilon_reference_for_flash_and_27b_heads() {
    let metal = ClefMetal::new().expect("explicit DeltaNet preparation device");
    let n = 4;
    let dim = 128;
    let kh = 16;
    for vh in [32, 48] {
        let width = (2 * kh + vh) * dim;
        let scales = [0.0, 1.0 / 65536.0, 1.0 / 1024.0, 0.5];
        let mut conv_values = vec![0.0; n * width];
        for (t, scale) in scales.into_iter().enumerate() {
            for h in 0..kh {
                for i in 0..dim {
                    conv_values[t * width + h * dim + i] =
                        ((i + h) % 7) as f32 * scale - 3.0 * scale;
                    conv_values[t * width + (kh + h) * dim + i] =
                        ((i * 3 + h) % 11) as f32 * scale - 5.0 * scale;
                }
            }
        }
        let conv = Guarded::new(&metal, &conv_values);
        let alpha = Guarded::new(&metal, &vec![0.0; n * vh]);
        let beta_in = Guarded::new(&metal, &vec![0.0; n * vh]);
        let a = Guarded::new(&metal, &vec![-1.0; vh]);
        let dt = Guarded::new(&metal, &vec![0.0; vh]);
        let q = Guarded::new(&metal, &vec![SENTINEL; n * kh * dim]);
        let k = Guarded::new(&metal, &vec![SENTINEL; n * kh * dim]);
        let decay = Guarded::new(&metal, &vec![SENTINEL; n * vh]);
        let beta = Guarded::new(&metal, &vec![SENTINEL; n * vh]);
        let kq = Guarded::new(&metal, &vec![SENTINEL; n * kh]);
        let batch = metal.batch();
        batch
            .delta_prep(
                &conv.view,
                &alpha.view,
                &beta_in.view,
                &a.view,
                &dt.view,
                &q.view,
                &k.view,
                &decay.view,
                &beta.view,
                &kq.view,
                n,
                kh,
                vh,
                dim,
                width,
            )
            .unwrap();
        batch.commit_wait().unwrap();
        let query = q.read();
        let key = k.read();
        let dots = kq.read();
        let compare = |actual: f32, expected: f64, name: &str| {
            assert!(
                actual.is_finite()
                    && (f64::from(actual) - expected).abs() <= 2e-7 + 3e-6 * expected.abs(),
                "{name}: {actual} vs {expected}, value heads={vh}"
            );
        };
        for t in 0..n {
            for h in 0..kh {
                let q_at = t * width + h * dim;
                let k_at = t * width + (kh + h) * dim;
                let norm = |at: usize| {
                    (conv_values[at..at + dim]
                        .iter()
                        .map(|&x| f64::from(x).powi(2))
                        .sum::<f64>()
                        + 1e-6)
                        .sqrt()
                };
                let q_denominator = norm(q_at) * (dim as f64).sqrt();
                let k_denominator = norm(k_at);
                let mut expected_dot = 0.0;
                for i in 0..dim {
                    let expected_q = f64::from(conv_values[q_at + i]) / q_denominator;
                    let expected_k = f64::from(conv_values[k_at + i]) / k_denominator;
                    let at = (t * kh + h) * dim + i;
                    compare(query[at], expected_q, "query");
                    compare(key[at], expected_k, "key");
                    expected_dot += expected_q * expected_k;
                }
                compare(dots[t * kh + h], expected_dot, "key/query dot");
            }
        }
        for value in decay.read() {
            compare(value, 0.5, "decay");
        }
        for value in beta.read() {
            compare(value, 0.5, "beta");
        }
        for input in [&conv, &alpha, &beta_in, &a, &dt] {
            input.check();
        }
    }
}

fn plan(case: &Case, n: usize) -> ClefDeltaPlan {
    ClefDeltaPlan::new(
        n,
        case.kh,
        case.vh,
        128,
        case.reordered,
        case.width,
        case.offset,
    )
    .unwrap()
}

fn close(actual: &[f32], expected: &[f64], label: &str) {
    assert_eq!(actual.len(), expected.len());
    // Acceptance bounds, not a claim about measured hardware accuracy. Full
    // model logit parity and timing remain separate requirements for promotion.
    for (i, (&actual, &expected)) in actual.iter().zip(expected).enumerate() {
        let bound = 2e-5 + 2e-4 * expected.abs();
        assert!(
            actual.is_finite() && (f64::from(actual) - expected).abs() <= bound,
            "{label}[{i}] {actual} vs {expected}; bound {bound}"
        );
    }
}

fn run(metal: &ClefMetal, case: &Case, chunks: &[usize], blocked: bool) -> (Vec<f32>, Vec<f32>) {
    assert_eq!(chunks.iter().sum::<usize>(), case.n);
    let state = Guarded::new(metal, &case.state);
    let mut all_output = Vec::new();
    let mut first = 0;
    for &n in chunks {
        assert!(n > 0);
        let q_values = &case.q[first * case.kh * 128..(first + n) * case.kh * 128];
        let k_values = &case.k[first * case.kh * 128..(first + n) * case.kh * 128];
        let q = Guarded::new(metal, q_values);
        let k = Guarded::new(metal, k_values);
        let conv_values = &case.conv[first * case.width..(first + n) * case.width];
        let conv = Guarded::new(metal, conv_values);
        let decay_values = &case.decay[first * case.vh..(first + n) * case.vh];
        let decay = Guarded::new(metal, decay_values);
        let beta_values = &case.beta[first * case.vh..(first + n) * case.vh];
        let beta = Guarded::new(metal, beta_values);
        let dots = q_values
            .chunks(128)
            .zip(k_values.chunks(128))
            .map(|(q, k)| q.iter().zip(k).map(|(&q, &k)| q * k).sum::<f32>())
            .collect::<Vec<_>>();
        let kq = Guarded::new(metal, &dots);
        let output = Guarded::new(metal, &vec![SENTINEL; n * case.vh * 128]);
        let workspace = Guarded::new(metal, &vec![SENTINEL; plan(case, n).workspace_bytes / 4]);
        let batch = metal.batch();
        if blocked {
            batch
                .delta_scan_block(
                    &q.view,
                    &k.view,
                    &conv.view,
                    &decay.view,
                    &beta.view,
                    &state.view,
                    &output.view,
                    &workspace.view,
                    n,
                    case.kh,
                    case.vh,
                    128,
                    case.reordered,
                    case.width,
                    case.offset,
                )
                .unwrap();
        } else {
            batch
                .delta_scan(
                    &q.view,
                    &k.view,
                    &conv.view,
                    &decay.view,
                    &beta.view,
                    &kq.view,
                    &state.view,
                    &output.view,
                    n,
                    case.kh,
                    case.vh,
                    128,
                    case.reordered,
                    case.width,
                    case.offset,
                )
                .unwrap();
        }
        batch.commit_wait().unwrap();
        all_output.extend(output.read());
        for (buffer, expected) in [
            (&q, q_values),
            (&k, k_values),
            (&conv, conv_values),
            (&decay, decay_values),
            (&beta, beta_values),
            (&kq, dots.as_slice()),
        ] {
            assert_eq!(buffer.read(), expected, "input buffer changed");
        }
        state.check();
        workspace.check();
        first += n;
    }
    (all_output, state.read())
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn block_delta_matches_dense_oracle_and_sequential_with_tails_and_head_orders() {
    let metal = metal();
    for (n, kh, vh, reordered) in [
        (1, 1, 1, false),
        (15, 1, 1, false),
        (31, 2, 4, false),
        (32, 2, 4, true),
        (33, 2, 4, false),
        (65, 2, 4, true),
    ] {
        let case = Case::new(n, kh, vh, reordered);
        let oracle = serial(&case);
        let sequential = run(&metal, &case, &[n], false);
        let block = run(&metal, &case, &[n], true);
        close(&sequential.0, &oracle.0, "sequential output");
        close(&sequential.1, &oracle.1, "sequential state");
        close(&block.0, &oracle.0, "block output");
        close(&block.1, &oracle.1, "block state");
        close(
            &block.0,
            &sequential
                .0
                .iter()
                .map(|&x| f64::from(x))
                .collect::<Vec<_>>(),
            "baseline output",
        );
        close(
            &block.1,
            &sequential
                .1
                .iter()
                .map(|&x| f64::from(x))
                .collect::<Vec<_>>(),
            "baseline state",
        );
        let repeated = run(&metal, &case, &[n], true);
        assert!(
            repeated
                .0
                .iter()
                .zip(&block.0)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        assert!(
            repeated
                .1
                .iter()
                .zip(&block.1)
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn block_delta_carries_state_across_non_aligned_chunks_and_zero_decay() {
    let metal = metal();
    for reordered in [false, true] {
        let mut case = Case::new(65, 2, 4, reordered);
        // Preserve initial state across a whole block, then test resets exactly
        // on either side of its boundary and underflowing cumulative products.
        case.decay.fill(1.0);
        case.decay[31 * case.vh] = 0.0;
        case.decay[32 * case.vh + 1] = 0.0;
        for t in 0..case.n {
            case.decay[t * case.vh + 2] = 2.0f32.powi(-40);
        }
        let oracle = serial(&case);
        let whole = run(&metal, &case, &[65], true);
        close(&whole.0, &oracle.0, "whole output");
        close(&whole.1, &oracle.1, "whole state");
        for chunks in [vec![33, 32], vec![15, 17, 31, 2], vec![1; 65]] {
            let chunked = run(&metal, &case, &chunks, true);
            close(&chunked.0, &oracle.0, "chunked output");
            close(&chunked.1, &oracle.1, "chunked state");
            close(
                &chunked.0,
                &whole.0.iter().map(|&x| f64::from(x)).collect::<Vec<_>>(),
                "chunk comparison",
            );
        }
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn block_delta_direct_solve_handles_correlated_keys_without_resets() {
    let metal = metal();
    for reordered in [false, true] {
        let case = Case::correlated(155, 2, 4, reordered);
        let oracle = serial(&case);
        let sequential = run(&metal, &case, &[155], false);
        close(&sequential.0, &oracle.0, "correlated sequential output");
        close(&sequential.1, &oracle.1, "correlated sequential state");
        for chunks in [vec![155], vec![31, 33, 64, 27]] {
            let actual = run(&metal, &case, &chunks, true);
            close(&actual.0, &oracle.0, "correlated block output");
            close(&actual.1, &oracle.1, "correlated block state");
            close(
                &actual.0,
                &sequential
                    .0
                    .iter()
                    .map(|&x| f64::from(x))
                    .collect::<Vec<_>>(),
                "correlated baseline output",
            );
            close(
                &actual.1,
                &sequential
                    .1
                    .iter()
                    .map(|&x| f64::from(x))
                    .collect::<Vec<_>>(),
                "correlated baseline state",
            );
            let worst = actual
                .0
                .iter()
                .zip(&sequential.0)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f32, f32::max);
            eprintln!(
                "correlated block Delta: reordered={reordered} chunks={chunks:?} max_abs_delta={worst:.8e}"
            );
        }
    }
}

#[test]
#[ignore = "requires explicit Metal hardware validation"]
fn block_delta_rejects_short_buffers_before_encoding_and_empty_is_a_noop() {
    let metal = metal();
    let case = Case::new(2, 2, 4, false);
    let plan = plan(&case, case.n);
    let sizes = [
        plan.query_key_bytes,
        plan.query_key_bytes,
        plan.conv_bytes,
        plan.decay_beta_bytes,
        plan.decay_beta_bytes,
        plan.state_bytes,
        plan.output_bytes,
        plan.workspace_bytes,
    ];
    let buffers = sizes.map(|bytes| metal.buffer(bytes));
    for short in 0..buffers.len() {
        let undersized = buffers[short].view(0, sizes[short] - 4).unwrap();
        let selected = (0..buffers.len())
            .map(|i| if i == short { &undersized } else { &buffers[i] })
            .collect::<Vec<_>>();
        let batch = metal.batch();
        assert!(
            batch
                .delta_scan_block(
                    selected[0],
                    selected[1],
                    selected[2],
                    selected[3],
                    selected[4],
                    selected[5],
                    selected[6],
                    selected[7],
                    case.n,
                    case.kh,
                    case.vh,
                    128,
                    false,
                    case.width,
                    case.offset
                )
                .is_err(),
            "short buffer {short}"
        );
        assert!(!batch.dispatched.get());
        batch.commit_wait().unwrap();
    }
    let empty = metal.buffer(16).view(0, 0).unwrap();
    let batch = metal.batch();
    batch
        .delta_scan_block(
            &empty,
            &empty,
            &empty,
            &empty,
            &empty,
            &empty,
            &empty,
            &empty,
            0,
            2,
            4,
            128,
            true,
            case.width,
            case.offset,
        )
        .unwrap();
    assert!(!batch.dispatched.get());
    batch.commit_wait().unwrap();
}
