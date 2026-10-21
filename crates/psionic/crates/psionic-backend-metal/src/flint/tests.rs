//! Each Flint kernel against a CPU reference on random data.
use super::*;

fn bf16(x: f32) -> u16 {
    let bits = x.to_bits();
    ((bits + 0x7fff + ((bits >> 16) & 1)) >> 16) as u16
}
fn f32_of(b: u16) -> f32 {
    f32::from_bits((b as u32) << 16)
}
fn round(x: f32) -> f32 {
    f32_of(bf16(x))
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
    }
    fn vec(&mut self, n: usize, scale: f32) -> Vec<f32> {
        (0..n).map(|_| round(self.unit() * scale)).collect()
    }
}

struct Gpu {
    device: Device,
    queue: CommandQueue,
    pipelines: BTreeMap<&'static str, ComputePipelineState>,
}

impl Gpu {
    fn new() -> Option<Self> {
        let device = Device::system_default()?;
        let pipelines = compile(&device).expect("Flint kernels compile");
        Some(Self {
            queue: device.new_command_queue(),
            device,
            pipelines,
        })
    }
    fn bf16s(&self, values: &[f32]) -> Buffer {
        let raw: Vec<u16> = values.iter().map(|v| bf16(*v)).collect();
        self.bytes(as_bytes(&raw))
    }
    fn f32s(&self, values: &[f32]) -> Buffer {
        self.bytes(as_bytes(values))
    }
    fn u32s(&self, values: &[u32]) -> Buffer {
        self.bytes(as_bytes(values))
    }
    fn bytes(&self, values: &[u8]) -> Buffer {
        let buffer = shared_buffer(&self.device, values.len());
        // SAFETY: the new buffer holds at least `values.len()` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(values.as_ptr(), buffer.contents().cast(), values.len())
        };
        buffer
    }
    fn zeros(&self, bytes: usize) -> Buffer {
        shared_buffer(&self.device, bytes)
    }
    fn run(&self, encode: impl FnOnce(&Pass<'_>)) {
        let command = self.queue.new_command_buffer();
        let encoder = command.new_compute_command_encoder();
        let pass = Pass::new(encoder, &self.pipelines);
        encode(&pass);
        pass.encoder.borrow().end_encoding();
        command.commit();
        command.wait_until_completed();
        assert_eq!(command.status(), metal::MTLCommandBufferStatus::Completed);
    }
}

fn as_bytes<T: Copy>(values: &[T]) -> &[u8] {
    // SAFETY: plain numeric values without padding.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}
fn read_bf16(buffer: &Buffer, n: usize) -> Vec<f32> {
    // SAFETY: the buffer holds at least n BF16 values written by a completed command.
    let raw = unsafe { std::slice::from_raw_parts(buffer.contents().cast::<u16>(), n) };
    raw.iter().map(|b| f32_of(*b)).collect()
}
fn read<T: Copy>(buffer: &Buffer, n: usize) -> Vec<T> {
    // SAFETY: as above, for plain values.
    unsafe { std::slice::from_raw_parts(buffer.contents().cast::<T>(), n).to_vec() }
}

/// A random group-64 INT4 matrix in MLX layout and its dequantized values.
struct Matrix {
    codes: Vec<u8>,
    scales: Vec<f32>,
    biases: Vec<f32>,
}
fn matrix(rng: &mut Rng, rows: usize, cols: usize) -> Matrix {
    let groups = cols / 64;
    let scales = (0..rows * groups)
        .map(|_| round(0.01 + 0.02 * rng.unit().abs()))
        .collect::<Vec<_>>();
    let biases = (0..rows * groups)
        .map(|_| round(-0.08 * rng.unit().abs()))
        .collect::<Vec<_>>();
    let mut codes = vec![0u8; rows * cols / 2];
    for r in 0..rows {
        for c in 0..cols {
            let q = (rng.next() % 16) as u8;
            codes[(r * cols + c) / 2] |= q << ((c % 2) * 4);
        }
    }
    Matrix {
        codes,
        scales,
        biases,
    }
}
impl Matrix {
    fn upload(&self, gpu: &Gpu) -> [Buffer; 3] {
        [
            gpu.bytes(&self.codes),
            gpu.bf16s(&self.scales),
            gpu.bf16s(&self.biases),
        ]
    }
    /// Product with weights rounded to BF16 as MLX's qmm dequantizes them.
    fn matvec(&self, rows: usize, x: &[f32]) -> Vec<f32> {
        let cols = x.len();
        let groups = cols / 64;
        (0..rows)
            .map(|r| {
                (0..cols)
                    .map(|c| {
                        let q = (self.codes[(r * cols + c) / 2] >> ((c % 2) * 4)) & 15;
                        let g = r * groups + c / 64;
                        let w = round(round(self.scales[g] * q as f32) + self.biases[g]);
                        w as f64 * x[c] as f64
                    })
                    .sum::<f64>() as f32
            })
            .collect()
    }
    /// The kernel's own arithmetic: per group, s * sum(q x) + b * sum(x), where
    /// sum(x) adds BF16-rounded four-value partials as rec's `load16` does.
    fn matvec_rec(&self, rows: usize, x: &[f32]) -> Vec<f32> {
        let cols = x.len();
        let groups = cols / 64;
        (0..rows)
            .map(|r| {
                let mut total = 0f64;
                for g in 0..groups {
                    let (mut qx, mut sx) = (0f64, 0f64);
                    for c in g * 64..(g + 1) * 64 {
                        let q = (self.codes[(r * cols + c) / 2] >> ((c % 2) * 4)) & 15;
                        qx += q as f64 * x[c] as f64;
                    }
                    for c in (g * 64..(g + 1) * 64).step_by(4) {
                        sx += round(round(round(x[c] + x[c + 1]) + x[c + 2]) + x[c + 3]) as f64;
                    }
                    let gi = r * groups + g;
                    total += self.scales[gi] as f64 * qx + self.biases[gi] as f64 * sx;
                }
                total as f32
            })
            .collect()
    }
}
fn bind3(pass: &Pass<'_>, first: u64, b: &[Buffer; 3]) {
    pass.buf(first, &b[0], 0)
        .buf(first + 1, &b[1], 0)
        .buf(first + 2, &b[2], 0);
}
fn close(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() <= tolerance * (1.0 + b.abs())
}

#[test]
fn int4_matvec_with_residual_matches_cpu() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(7);
    let m = matrix(&mut rng, 2048, 4096);
    let x = rng.vec(4096, 1.0);
    let h = rng.vec(2048, 4.0);
    let wb = m.upload(&gpu);
    let xb = gpu.bf16s(&x);
    let hb = gpu.bf16s(&h);
    gpu.run(|p| {
        p.kernel("flint_residual_4096");
        bind3(p, 0, &wb);
        p.buf(3, &xb, 0).buf(4, &hb, 0).groups((64, 1), 256);
    });
    let got = read_bf16(&hb, 2048);
    let want = m.matvec_rec(2048, &x);
    for i in 0..2048 {
        let expected = round(h[i] + round(want[i]));
        // Two BF16 steps of the larger operand: the sum can cancel.
        let ulp = (h[i].abs() + want[i].abs()) / 64.0;
        assert!(
            (got[i] - expected).abs() <= ulp + 1e-3,
            "row {i}: {} vs {expected}",
            got[i]
        );
    }
}

#[test]
fn int4_gemm_matches_cpu_in_each_mode() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(11);
    let (m, n, k) = (37, 96, 128);
    let w = matrix(&mut rng, n, k);
    let w2 = matrix(&mut rng, n, k);
    let x = rng.vec(m * k, 1.0);
    let base = rng.vec(m * n, 2.0);
    let wb = w.upload(&gpu);
    let w2b = w2.upload(&gpu);
    let xb = gpu.bf16s(&x);
    let rows = gpu.u32s(&[0]);
    let tiles = gpu.u32s(&[0]);
    let kernels: Vec<&str> = ["flint_gemm_q4", "flint_gemm_q4_tensor"]
        .into_iter()
        .filter(|k| gpu.pipelines.contains_key(k))
        .collect();
    for (kernel, mode) in kernels.iter().flat_map(|k| (0..3u32).map(move |m| (*k, m))) {
        let yb = gpu.bf16s(&base);
        let params = GemmParams {
            m: m as u32,
            n: n as u32,
            k: k as u32,
            ldx: k as u32,
            ldy: n as u32,
            mode,
            gathered: 0,
            tiles: 0,
            expert_stride_w: 0,
            expert_stride_g: 0,
        };
        gpu.run(|p| {
            p.kernel(kernel).buf(0, &xb, 0);
            bind3(p, 1, &wb);
            bind3(p, 4, &w2b);
            p.buf(7, &yb, 0)
                .val(8, &params)
                .buf(9, &rows, 0)
                .buf(10, &tiles, 0)
                .groups(
                    (n.div_ceil(if mode == 2 { 32 } else { 64 }), m.div_ceil(64)),
                    128,
                );
        });
        let got = read_bf16(&yb, m * n);
        for r in 0..m {
            let g = w.matvec(n, &x[r * k..(r + 1) * k]);
            let u = w2.matvec(n, &x[r * k..(r + 1) * k]);
            for c in 0..n {
                let expected = match mode {
                    0 => round(g[c]),
                    1 => round(base[r * n + c] + round(g[c])),
                    _ => {
                        let gate = round(g[c]);
                        let sig = round(1.0 / (1.0 + (-gate).exp()));
                        round(round(gate * sig) * round(u[c]))
                    }
                };
                assert!(
                    close(got[r * n + c], expected, 1.0 / 64.0),
                    "{kernel} mode {mode} ({r},{c}): {} vs {expected}",
                    got[r * n + c]
                );
            }
        }
    }
}

#[test]
fn router_product_applies_sigmoid() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(29);
    let (m, n, k) = (70, 256, 2048);
    let w = rng.vec(n * k, 0.05);
    let x = rng.vec(m * k, 1.0);
    let (wb, xb) = (gpu.bf16s(&w), gpu.bf16s(&x));
    let rows = gpu.u32s(&[0]);
    let tiles = gpu.u32s(&[0]);
    let params = GemmParams {
        m: m as u32,
        n: n as u32,
        k: k as u32,
        ldx: k as u32,
        ldy: n as u32,
        mode: 3,
        gathered: 0,
        tiles: 0,
        expert_stride_w: 0,
        expert_stride_g: 0,
    };
    for kernel in ["flint_gemm_q4", "flint_gemm_q4_tensor"] {
        if !gpu.pipelines.contains_key(kernel) {
            continue;
        }
        let probs = gpu.zeros(m * n * 4);
        gpu.run(|p| {
            p.kernel(kernel)
                .buf(0, &xb, 0)
                .buf(1, &wb, 0)
                .buf(2, &wb, 0)
                .buf(3, &wb, 0)
                .buf(4, &wb, 0)
                .buf(5, &wb, 0)
                .buf(6, &wb, 0)
                .buf(7, &probs, 0)
                .val(8, &params)
                .buf(9, &rows, 0)
                .buf(10, &tiles, 0)
                .groups((n.div_ceil(64), m.div_ceil(64)), 128);
        });
        let got: Vec<f32> = read(&probs, m * n);
        for r in 0..m {
            for e in 0..n {
                let dot: f64 = (0..k)
                    .map(|i| w[e * k + i] as f64 * x[r * k + i] as f64)
                    .sum();
                let want = 1.0 / (1.0 + (-round(dot as f32)).exp());
                assert!(
                    (got[r * n + e] - want).abs() < 4e-3,
                    "{kernel} ({r},{e}): {} vs {want}",
                    got[r * n + e]
                );
            }
        }
    }
}

#[test]
fn top_k_ranks_with_lower_index_ties_and_flags_non_finite() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(37);
    let vocab = 150_144usize;
    let mut logits: Vec<f32> = (0..vocab).map(|_| round(rng.unit() * 8.0)).collect();
    for i in [77usize, 9_000, 149_999] {
        logits[i] = 12.0; // a three-way tie at the top
    }
    let slices = vocab.div_ceil(2048);
    let k = 20usize;
    for poison in [false, true] {
        let mut values = logits.clone();
        if poison {
            values[5_000] = f32::NAN;
        }
        let lb = gpu.f32s(&values);
        let (cv, ci) = (gpu.zeros(slices * 32 * 4), gpu.zeros(slices * 32 * 4));
        let (tv, ti, bad) = (gpu.zeros(32 * 4), gpu.zeros(32 * 4), gpu.zeros(16));
        gpu.run(|p| {
            p.kernel("flint_topk_partial")
                .buf(0, &lb, 0)
                .buf(1, &cv, 0)
                .buf(2, &ci, 0)
                .buf(3, &bad, 0)
                .val(4, &(vocab as u32))
                .val(5, &(k as u32))
                .groups((slices, 1), 256);
            p.kernel("flint_topk_merge")
                .buf(0, &cv, 0)
                .buf(1, &ci, 0)
                .buf(2, &tv, 0)
                .buf(3, &ti, 0)
                .val(4, &((slices * k) as u32))
                .val(5, &(k as u32))
                .groups((1, 1), 256);
        });
        let flag: Vec<u32> = read(&bad, 1);
        assert_eq!(flag[0] != 0, poison);
        if poison {
            continue;
        }
        let mut order: Vec<(usize, f32)> = logits.iter().copied().enumerate().collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let ids: Vec<u32> = read(&ti, k);
        let vals: Vec<f32> = read(&tv, k);
        for r in 0..k {
            assert_eq!((ids[r] as usize, vals[r]), order[r], "rank {r}");
        }
    }
}

/// Sampling::choose_ranked's rule over ranked candidates, in FP64.
fn cpu_choose_ranked(values: &[f32], temperature: f64, top_p: f64, draw: f64) -> usize {
    let max = values[0] as f64;
    let w: Vec<f64> = values
        .iter()
        .map(|v| ((*v as f64 - max) / temperature).exp())
        .collect();
    let total: f64 = w.iter().sum();
    let (mut cumulative, mut keep) = (0.0, 0);
    for x in &w {
        cumulative += x / total;
        keep += 1;
        if cumulative >= top_p {
            break;
        }
    }
    let mut d = draw * w[..keep].iter().sum::<f64>();
    for (i, x) in w[..keep].iter().enumerate() {
        d -= x;
        if d <= 0.0 {
            return i;
        }
    }
    keep - 1
}

#[test]
fn gpu_sampling_follows_the_ranked_choice() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(41);
    let mut agree = 0;
    let cases = 400;
    for case in 0..cases {
        let k = 20;
        let mut values: Vec<f32> = (0..k).map(|_| round(rng.unit() * 6.0)).collect();
        values.sort_by(|a, b| b.total_cmp(a));
        let ids: Vec<u32> = (0..k as u32).map(|i| 1000 + i).collect();
        let draw = (rng.next() >> 11) as f64 / (1u64 << 53) as f64;
        let (temperature, top_p) = if case % 2 == 0 {
            (1.0, 0.95)
        } else {
            (0.7, 0.8)
        };
        let (tv, ti) = (gpu.f32s(&values), gpu.u32s(&ids));
        let greedy = gpu.u32s(&[7]);
        let draws = gpu.f32s(&[draw as f32]);
        let (slot, out) = (gpu.zeros(16), gpu.zeros(16));
        let params = SampleParams {
            greedy: 0,
            temperature,
            top_p,
            k: k as u32,
            step: 0,
        };
        gpu.run(|p| {
            p.kernel("flint_sample")
                .buf(0, &tv, 0)
                .buf(1, &ti, 0)
                .buf(2, &greedy, 0)
                .buf(3, &draws, 0)
                .buf(4, &slot, 0)
                .buf(5, &out, 0)
                .val(6, &params)
                .groups((1, 1), 32);
        });
        let got: Vec<u32> = read(&out, 1);
        let next: Vec<u32> = read(&slot, 1);
        assert_eq!(got, next, "the chosen token feeds the next step");
        let want = 1000 + cpu_choose_ranked(&values, temperature as f64, top_p as f64, draw) as u32;
        agree += usize::from(got[0] == want);
    }
    // FP32 against FP64 can differ only when a draw lands on a boundary.
    assert!(agree >= cases - 2, "{agree} of {cases}");
}

#[test]
fn embedding_dequantizes_int8_rows() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(3);
    let vocab = 8;
    let codes: Vec<u8> = (0..vocab * 2048).map(|_| rng.next() as u8).collect();
    let scales = rng.vec(vocab * 32, 0.01);
    let biases = rng.vec(vocab * 32, 0.5);
    let tokens = [5u32, 2];
    let wb = [gpu.bytes(&codes), gpu.bf16s(&scales), gpu.bf16s(&biases)];
    let hb = gpu.zeros(2 * 2048 * 2);
    let tb = gpu.u32s(&tokens);
    gpu.run(|p| {
        p.kernel("flint_embed8");
        bind3(p, 0, &wb);
        p.buf(3, &hb, 0)
            .buf(4, &tb, 0)
            .val(5, &2u32)
            .threads((2048, 2), 256);
    });
    let got = read_bf16(&hb, 4096);
    for (t, token) in tokens.iter().enumerate() {
        for i in 0..2048 {
            let row = *token as usize;
            let g = row * 32 + i / 64;
            let want = round(round(scales[g] * codes[row * 2048 + i] as f32) + biases[g]);
            assert_eq!(got[t * 2048 + i], want);
        }
    }
}

fn cpu_route(probs: &[f32]) -> Vec<(usize, f32)> {
    let mut order: Vec<_> = probs.iter().copied().enumerate().collect();
    order.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    order.truncate(6);
    let sum: f32 = order.iter().map(|x| x.1).sum::<f32>() + 1e-20;
    order.iter().map(|(i, p)| (*i, p / sum)).collect()
}

#[test]
fn routing_picks_top_six_with_lower_index_ties() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(5);
    let rows = 9;
    let mut probs: Vec<f32> = (0..rows * 256).map(|_| rng.unit().abs()).collect();
    // Force ties: equal maxima at different indices.
    probs[3] = 2.0;
    probs[200] = 2.0;
    probs[256 + 7] = 1.5;
    probs[256 + 9] = 1.5;
    let pb = gpu.f32s(&probs);
    let ids = gpu.zeros(rows * 6 * 4);
    let weights = gpu.zeros(rows * 6 * 4);
    let counts = gpu.zeros(256 * 4);
    gpu.run(|p| {
        p.kernel("flint_rows_route")
            .buf(0, &pb, 0)
            .buf(1, &ids, 0)
            .buf(2, &weights, 0)
            .buf(3, &counts, 0)
            .val(4, &(rows as u32))
            .groups((rows.div_ceil(8), 1), 256);
    });
    let got_ids: Vec<u32> = read(&ids, rows * 6);
    let got_w: Vec<f32> = read(&weights, rows * 6);
    let got_counts: Vec<u32> = read(&counts, 256);
    let mut want_counts = vec![0u32; 256];
    for r in 0..rows {
        let want = cpu_route(&probs[r * 256..(r + 1) * 256]);
        for (rank, (id, weight)) in want.iter().enumerate() {
            assert_eq!(got_ids[r * 6 + rank] as usize, *id, "row {r} rank {rank}");
            assert!(close(got_w[r * 6 + rank], *weight, 1e-6));
            want_counts[*id] += 1;
        }
    }
    assert_eq!(got_counts, want_counts);
}

fn cpu_head_norm(x: &[f32], w: &[f32], rotary: bool, position: usize) -> Vec<f32> {
    let ms = x.iter().map(|v| v * v).sum::<f32>() / 128.0;
    let inv = 1.0 / (ms + 1e-6).sqrt();
    let n: Vec<f32> = (0..128).map(|d| round(w[d] * round(x[d] * inv))).collect();
    if !rotary {
        return n;
    }
    let mut out = n.clone();
    for i in 0..64 {
        let theta = position as f64 / 10000f64.powf(i as f64 / 64.0);
        let (s, c) = (theta.sin() as f32, theta.cos() as f32);
        out[i] = round(n[i] * c - n[i + 64] * s);
        out[i + 64] = round(n[i] * s + n[i + 64] * c);
    }
    out
}

#[test]
fn head_norm_and_rope_match_cpu() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(13);
    let rows = 3;
    let qkv = rng.vec(rows * 5120, 3.0);
    let qn = rng.vec(128, 1.5);
    let kn = rng.vec(128, 1.5);
    for rotary in [false, true] {
        let b = gpu.bf16s(&qkv);
        let (qb, kb) = (gpu.bf16s(&qn), gpu.bf16s(&kn));
        let params = RowParams {
            start: 700,
            cap: 513,
            rotary: rotary as u32,
            global: !rotary as u32,
            count: rows as u32,
        };
        gpu.run(|p| {
            p.kernel("flint_rows_head_norm")
                .buf(0, &b, 0)
                .buf(1, &qb, 0)
                .buf(2, &kb, 0)
                .val(3, &params)
                .groups(((rows * 36).div_ceil(8), 1), 256);
        });
        let got = read_bf16(&b, rows * 5120);
        for r in 0..rows {
            for head in 0..36 {
                let at = r * 5120 + head * 128;
                let w = if head < 32 { &qn } else { &kn };
                let want = cpu_head_norm(&qkv[at..at + 128], w, rotary, 700 + r);
                for d in 0..128 {
                    assert!(
                        close(got[at + d], want[d], 1.0 / 64.0),
                        "rotary {rotary} row {r} head {head} dim {d}: {} vs {}",
                        got[at + d],
                        want[d]
                    );
                }
            }
        }
    }
}

/// Softmax attention with a sink logit over `keys`, for one query head.
fn cpu_attention(q: &[f32], keys: &[&[f32]], values: &[&[f32]], sink: f32) -> Vec<f32> {
    let scale = 1.0 / (128f32).sqrt();
    let scores: Vec<f32> = keys
        .iter()
        .map(|k| q.iter().zip(k.iter()).map(|(a, b)| a * b).sum::<f32>() * scale)
        .collect();
    let top = scores.iter().copied().fold(sink, f32::max);
    let z = (sink - top).exp() + scores.iter().map(|s| (s - top).exp()).sum::<f32>();
    (0..128)
        .map(|d| {
            scores
                .iter()
                .zip(values)
                .map(|(s, v)| (s - top).exp() * v[d])
                .sum::<f32>()
                / z
        })
        .collect()
}

#[test]
fn decode_attention_with_sink_matches_cpu() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(17);
    for (global, count) in [(false, 300usize), (true, 1500), (true, 1024)] {
        let cap = if global { 2048 } else { 513 };
        let q = rng.vec(4096, 1.0);
        let kv = rng.vec(2 * 4 * cap * 128, 1.0);
        let sinks = rng.vec(32, 2.0);
        let (parts, rounded, heads) = attention_schedule(global, count);
        let params = AttentionParams {
            count: count as u32,
            cap: cap as u32,
            parts: parts as u32,
            rounded: rounded as u32,
            heads: heads as u32,
        };
        let (qb, kvb, sb) = (gpu.bf16s(&q), gpu.bf16s(&kv), gpu.bf16s(&sinks));
        let partial = gpu.zeros(32 * 1024 * 128 * 4);
        let stats = gpu.zeros(32 * 1024 * 2 * 4);
        let out = gpu.zeros(4096 * 2);
        let tasks = (32 / heads) * parts;
        gpu.run(|p| {
            p.kernel("flint_attention_partial")
                .buf(0, &qb, 0)
                .buf(1, &kvb, 0)
                .buf(2, &sb, 0)
                .buf(3, &partial, 0)
                .buf(4, &stats, 0)
                .val(5, &params)
                .groups((tasks.div_ceil(8), 1), 256);
            p.kernel("flint_attention_merge")
                .buf(0, &out, 0)
                .buf(1, &partial, 0)
                .buf(2, &stats, 0)
                .val(3, &params)
                .groups((32, 1), 256);
        });
        let got = read_bf16(&out, 4096);
        for h in 0..32 {
            let kvh = h / 8;
            let keys: Vec<&[f32]> = (0..count)
                .map(|t| &kv[(kvh * cap + t) * 128..(kvh * cap + t + 1) * 128])
                .collect();
            let values: Vec<&[f32]> = (0..count)
                .map(|t| &kv[((4 + kvh) * cap + t) * 128..((4 + kvh) * cap + t + 1) * 128])
                .collect();
            let want = cpu_attention(&q[h * 128..(h + 1) * 128], &keys, &values, sinks[h]);
            for d in 0..128 {
                assert!(
                    (got[h * 128 + d] - want[d]).abs() < 0.02,
                    "count {count} head {h} dim {d}: {} vs {}",
                    got[h * 128 + d],
                    want[d]
                );
            }
        }
    }
}

#[test]
fn chunk_attention_reads_cache_and_chunk() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(19);
    let count = 40;
    let variants: Vec<(&str, usize, usize, usize)> = [
        ("flint_rows_attention", 8, 4, 256),
        ("flint_rows_attention_tensor", 64, 32, 128),
    ]
    .into_iter()
    .filter(|v| gpu.pipelines.contains_key(v.0))
    .collect();
    for ((global, start), (kernel, per, heads, threads)) in [(false, 600usize), (true, 70)]
        .into_iter()
        .flat_map(|case| variants.iter().map(move |v| (case, *v)))
    {
        let cap = if global { 256 } else { 513 };
        let qkv = rng.vec(count * 5120, 1.0);
        let kv = rng.vec(2 * 4 * cap * 128, 1.0);
        let sinks = rng.vec(32, 2.0);
        let params = RowParams {
            start: start as u32,
            cap: cap as u32,
            rotary: 0,
            global: global as u32,
            count: count as u32,
        };
        let (qb, kvb, sb) = (gpu.bf16s(&qkv), gpu.bf16s(&kv), gpu.bf16s(&sinks));
        let out = gpu.zeros(count * 4096 * 2);
        gpu.run(|p| {
            p.kernel(kernel)
                .buf(0, &qb, 0)
                .buf(1, &kvb, 0)
                .buf(2, &sb, 0)
                .buf(3, &out, 0)
                .val(4, &params)
                .val(5, &(WINDOW as u32))
                .groups((count.div_ceil(per), heads), threads);
        });
        let got = read_bf16(&out, count * 4096);
        for row in [0, 17, count - 1] {
            let position = start + row;
            let lo = if global {
                0
            } else {
                (position + 1).saturating_sub(WINDOW)
            };
            for h in [0, 9, 31] {
                let kvh = h / 8;
                let key = |t: usize| -> &[f32] {
                    if t >= start {
                        let at = (t - start) * 5120 + 4096 + kvh * 128;
                        &qkv[at..at + 128]
                    } else {
                        let slot = if global { t } else { t % cap };
                        &kv[(kvh * cap + slot) * 128..(kvh * cap + slot + 1) * 128]
                    }
                };
                let value = |t: usize| -> &[f32] {
                    if t >= start {
                        let at = (t - start) * 5120 + 4608 + kvh * 128;
                        &qkv[at..at + 128]
                    } else {
                        let slot = if global { t } else { t % cap };
                        &kv[((4 + kvh) * cap + slot) * 128..((4 + kvh) * cap + slot + 1) * 128]
                    }
                };
                let keys: Vec<&[f32]> = (lo..=position).map(key).collect();
                let values: Vec<&[f32]> = (lo..=position).map(value).collect();
                let q = &qkv[row * 5120 + h * 128..row * 5120 + (h + 1) * 128];
                let want = cpu_attention(q, &keys, &values, sinks[h]);
                for d in 0..128 {
                    let g = got[row * 4096 + h * 128 + d];
                    assert!(
                        (g - want[d]).abs() < 0.02,
                        "row {row} head {h} dim {d}: {g} vs {}",
                        want[d]
                    );
                }
            }
        }
    }
}

#[test]
fn expert_plan_groups_slots_by_expert() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(23);
    let rows = 50;
    let ids: Vec<u32> = (0..rows * 6).map(|_| (rng.next() % 40) as u32).collect();
    let mut counts = vec![0u32; 256];
    for id in &ids {
        counts[*id as usize] += 1;
    }
    let idb = gpu.u32s(&ids);
    let cb = gpu.u32s(&counts);
    let offsets = gpu.zeros(257 * 4);
    let tiles = gpu.zeros((1 + 3 * (rows * 6 / 64 + 257)) * 4);
    let slot_rows = gpu.zeros(rows * 6 * 4);
    let slot_of = gpu.zeros(rows * 6 * 4);
    gpu.run(|p| {
        p.kernel("flint_rows_plan")
            .buf(0, &cb, 0)
            .buf(1, &offsets, 0)
            .buf(2, &tiles, 0)
            .groups((1, 1), 32);
        p.kernel("flint_rows_scatter")
            .buf(0, &idb, 0)
            .buf(1, &offsets, 0)
            .buf(2, &slot_rows, 0)
            .buf(3, &slot_of, 0)
            .val(4, &(rows as u32))
            .threads((256, 1), 256);
    });
    let slot_rows: Vec<u32> = read(&slot_rows, rows * 6);
    let slot_of: Vec<u32> = read(&slot_of, rows * 6);
    let tiles: Vec<u32> = read(&tiles, 1 + 3 * (rows * 6 / 64 + 257));
    let after: Vec<u32> = read(&cb, 256);
    assert!(
        after.iter().all(|c| *c == 0),
        "counts reset for the next layer"
    );
    let mut covered = 0;
    for tile in 0..tiles[0] as usize {
        let (e, start, n) = (
            tiles[1 + tile * 3],
            tiles[2 + tile * 3],
            tiles[3 + tile * 3],
        );
        assert!((1..=64).contains(&n));
        for slot in start..start + n {
            let row = slot_rows[slot as usize] as usize;
            let rank = (0..6)
                .find(|r| slot_of[row * 6 + r] == slot)
                .expect("slot maps back");
            assert_eq!(ids[row * 6 + rank], e);
            covered += 1;
        }
    }
    assert_eq!(covered, rows * 6);
}

/// Times chunk attention on one long-context chunk. Run with
/// `cargo test --release -- --ignored attention_speed --nocapture`.
#[test]
#[ignore]
fn attention_speed() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(31);
    let (count, start, cap) = (2048usize, 14336usize, 16384usize);
    let qkv = gpu.bf16s(&rng.vec(count * 5120, 1.0));
    let kv = gpu.bf16s(&rng.vec(2 * 4 * cap * 128, 1.0));
    let sinks = gpu.bf16s(&rng.vec(32, 1.0));
    let out = gpu.zeros(count * 4096 * 2);
    let params = RowParams {
        start: start as u32,
        cap: cap as u32,
        rotary: 0,
        global: 1,
        count: count as u32,
    };
    for round in 0..3 {
        for (kernel, per, heads, threads) in [
            ("flint_rows_attention", 8, 4, 256),
            ("flint_rows_attention_tensor", 64, 32, 128),
        ] {
            if !gpu.pipelines.contains_key(kernel) {
                continue;
            }
            let command = gpu.queue.new_command_buffer();
            let encoder = command.new_compute_command_encoder();
            let pass = Pass::new(encoder, &gpu.pipelines);
            pass.kernel(kernel)
                .buf(0, &qkv, 0)
                .buf(1, &kv, 0)
                .buf(2, &sinks, 0)
                .buf(3, &out, 0)
                .val(4, &params)
                .val(5, &(WINDOW as u32))
                .groups((count.div_ceil(per), heads), threads);
            pass.encoder.borrow().end_encoding();
            command.commit();
            command.wait_until_completed();
            eprintln!(
                "round {round} {kernel}: {:.1} ms",
                gpu_seconds(command) * 1e3
            );
        }
    }
}

/// Times the two top-k stages on a vocabulary of logits. Run with
/// `cargo test --release -- --ignored top_k_speed --nocapture`.
#[test]
#[ignore]
fn top_k_speed() {
    let Some(gpu) = Gpu::new() else { return };
    let mut rng = Rng(43);
    let vocab = 150_144usize;
    let logits = gpu.f32s(&(0..vocab).map(|_| rng.unit() * 8.0).collect::<Vec<_>>());
    let slices = vocab.div_ceil(2048);
    let (cv, ci) = (gpu.zeros(slices * 32 * 4), gpu.zeros(slices * 32 * 4));
    let (tv, ti, bad) = (gpu.zeros(32 * 4), gpu.zeros(32 * 4), gpu.zeros(16));
    for round in 0..3 {
        for stage in ["flint_topk_partial", "flint_topk_merge"] {
            let command = gpu.queue.new_command_buffer();
            let encoder = command.new_compute_command_encoder();
            let pass = Pass::new(encoder, &gpu.pipelines);
            if stage == "flint_topk_partial" {
                pass.kernel(stage)
                    .buf(0, &logits, 0)
                    .buf(1, &cv, 0)
                    .buf(2, &ci, 0)
                    .buf(3, &bad, 0)
                    .val(4, &(vocab as u32))
                    .val(5, &20u32)
                    .groups((slices, 1), 256);
            } else {
                pass.kernel(stage)
                    .buf(0, &cv, 0)
                    .buf(1, &ci, 0)
                    .buf(2, &tv, 0)
                    .buf(3, &ti, 0)
                    .val(4, &((slices * 20) as u32))
                    .val(5, &20u32)
                    .groups((1, 1), 256);
            }
            pass.encoder.borrow().end_encoding();
            command.commit();
            command.wait_until_completed();
            eprintln!(
                "round {round} {stage}: {:.3} ms",
                gpu_seconds(command) * 1e3
            );
        }
    }
}
