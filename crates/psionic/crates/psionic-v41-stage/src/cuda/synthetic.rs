//! A tiny V4.1-shaped model in checkpoint form for CUDA stages: V4.1's
//! 40-layer layout (KV sources 2, 8, 14, 20; index sources through 36;
//! Engram on layers 1 and 14) with small shapes, FP8 32×32-block dense
//! weights, FP4 per-32 routed experts, and BF16 and F32 vectors.
//!
//! Reimplemented from W7's test model (`psionic-backend-cuda`'s
//! `v41_stage_tests.rs`, #12394), so a group of stage processes can run on a
//! small GPU without the checkpoint. The weights are generated, not loaded:
//! every process with the same seed builds the same model.

use std::{collections::BTreeMap, ops::Range};

use psionic_backend_cuda::{
    CudaBackend,
    v41_attention::V41AttentionGeometry,
    v41_mix::{V41HyperConfig, V41Router},
    v41_stage::{
        V41EngramHostTable, V41HostTensor, V41StageShape, V41TensorDtype, V41TensorSource,
    },
};
use psionic_models::{
    deepseek_v41_cpu::{
        engram::{CompressedTokenMap, EngramLayout, LayerLayout},
        quantization::{encode_fp4, encode_fp8},
    },
    deepseek_v41_weights::decode_f8_e8m0fnu,
};
use psionic_runtime::RuntimeError;
use tokenizers::Tokenizer;

use crate::Refused;

/// Tokens in the vocabulary, `t0` to `t63`.
pub const VOCAB: usize = 64;
/// The end-of-answer token.
pub const EOS: u32 = 1;
/// The pad token, which is also the prompt when the front sends none.
pub const PAD: u32 = 2;
/// The seed every stage process builds the model from.
pub const SEED: u64 = 11;

const HIDDEN: usize = 128;
const HEADS: usize = 4;
const HEAD_DIM: usize = 64;
const Q_RANK: usize = 64;
const O_GROUPS: usize = 2;
const O_RANK: usize = 32;
const INDEX_HEADS: usize = 4;
const INDEX_DIM: usize = 32;
const EXPERTS: usize = 16;
const EXPERT_WIDTH: usize = 64;
const STREAMS: usize = 4;
const TOP_BLOCKS: usize = 2;
const ENGRAM_WIDTH: usize = 32;

/// The model's shapes.
#[must_use]
pub fn shape() -> V41StageShape {
    V41StageShape {
        hidden: HIDDEN,
        q_rank: Q_RANK,
        o_groups: O_GROUPS,
        o_rank: O_RANK,
        attention: V41AttentionGeometry {
            heads: HEADS,
            head_dim: HEAD_DIM,
            rope_dim: 16,
            window: 8,
            index_heads: INDEX_HEADS,
            index_dim: INDEX_DIM,
            index_top_k: 4,
            norm_epsilon: 1e-20,
        },
        hc: V41HyperConfig {
            width: HIDDEN,
            ..V41HyperConfig::flash()
        },
        router: V41Router {
            experts: EXPERTS,
            ..V41Router::flash()
        },
        expert_width: EXPERT_WIDTH,
        swiglu_limit: 10.0,
        vocabulary: VOCAB,
        engram_width: ENGRAM_WIDTH,
        engram_heads: 8,
        candidate_blocks: TOP_BLOCKS,
        candidate_block_size: 8,
    }
}

fn primes(count: usize, from: u64) -> Vec<u64> {
    let mut out = Vec::new();
    let mut n = from;
    while out.len() < count {
        if (2..n).take_while(|d| d * d <= n).all(|d| n % d != 0) {
            out.push(n);
        }
        n += 1;
    }
    out
}

/// The Engram hash layout of layers 1 and 14.
#[must_use]
pub fn layout() -> EngramLayout {
    let mut layers = Vec::new();
    for (i, layer_id) in [1_u32, 14].into_iter().enumerate() {
        let primes = primes(24, 5 + 40 * i as u64);
        let mut offsets = Vec::new();
        let mut rows = 0;
        for p in &primes {
            offsets.push(rows);
            rows += p;
        }
        layers.push(LayerLayout {
            layer_id,
            primes,
            offsets,
            multipliers: [1_000_003, 999_983 * 2 + 1, 12_345_679, 7_654_321 * 2 + 1],
            table_rows: rows,
        });
    }
    EngramLayout {
        layers,
        max_ngram_size: 4,
        heads: 8,
        head_dim: ENGRAM_WIDTH,
        compressed_vocab_size: VOCAB,
    }
}

/// The tokenizer: `t0` to `t63`, each a word, and anything else `t0`.
///
/// # Errors
///
/// Only if the tokenizer can't be built.
pub fn tokenizer() -> Result<Tokenizer, Refused> {
    let vocab: BTreeMap<_, _> = (0..VOCAB).map(|i| (format!("t{i}"), i)).collect();
    let json = serde_json::json!({"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":vocab,"unk_token":"t0"}});
    Tokenizer::from_bytes(json.to_string()).map_err(|e| Refused(e.to_string()))
}

/// Every token its own compressed id.
///
/// # Errors
///
/// Only if the tokenizer can't be built.
pub fn token_map() -> Result<CompressedTokenMap, Refused> {
    CompressedTokenMap::build(&tokenizer()?, VOCAB).map_err(|e| Refused(e.to_string()))
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }

    /// Uniform in [-1, 1).
    fn unit(&mut self) -> f32 {
        (self.next() as f64 / (1u64 << 52) as f64 * 2.0 - 1.0) as f32
    }
}

fn scale_exponent(amax: f32, top: f32) -> u8 {
    let ratio = (amax.max(1e-30) / top).log2().ceil();
    (ratio as i32 + 127).clamp(1, 254) as u8
}

/// The model's tensors by checkpoint name.
pub struct Store {
    tensors: BTreeMap<String, V41HostTensor>,
}

impl V41TensorSource for Store {
    fn tensor(&self, name: &str) -> Result<V41HostTensor, RuntimeError> {
        self.tensors
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::Backend(format!("synthetic tensor {name} missing")))
    }
}

impl Store {
    fn insert(&mut self, name: String, dtype: V41TensorDtype, shape: Vec<usize>, bytes: Vec<u8>) {
        self.tensors.insert(
            name,
            V41HostTensor {
                dtype,
                shape,
                bytes,
            },
        );
    }

    fn f32s(&mut self, name: String, shape: Vec<usize>, values: &[f32]) {
        let bytes = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.insert(name, V41TensorDtype::F32, shape, bytes);
    }

    fn bf16s(&mut self, name: String, shape: Vec<usize>, values: &[f32]) {
        let bytes = values
            .iter()
            .flat_map(|v| half::bf16::from_f32(*v).to_bits().to_le_bytes())
            .collect();
        self.insert(name, V41TensorDtype::Bf16, shape, bytes);
    }

    /// FP8 with one scale per 32×32 block.
    fn fp8(&mut self, stem: &str, rows: usize, cols: usize, rng: &mut Rng, amplitude: f32) {
        let values: Vec<f32> = (0..rows * cols).map(|_| rng.unit() * amplitude).collect();
        let (br, bc) = (rows.div_ceil(32), cols / 32);
        let mut scales = vec![0_u8; br * bc];
        for (b, scale) in scales.iter_mut().enumerate() {
            let (r0, c0) = (b / bc * 32, b % bc * 32);
            let mut amax = 0.0_f32;
            for r in r0..(r0 + 32).min(rows) {
                for c in c0..c0 + 32 {
                    amax = amax.max(values[r * cols + c].abs());
                }
            }
            *scale = scale_exponent(amax, 448.0);
        }
        let codes = (0..rows * cols)
            .map(|i| {
                let s = decode_f8_e8m0fnu(scales[(i / cols / 32) * bc + (i % cols) / 32]);
                encode_fp8(values[i] / s).unwrap_or(0)
            })
            .collect();
        self.insert(
            format!("{stem}.weight"),
            V41TensorDtype::F8E4M3,
            vec![rows, cols],
            codes,
        );
        self.insert(
            format!("{stem}.scale"),
            V41TensorDtype::F8E8M0,
            vec![br, bc],
            scales,
        );
    }

    /// FP8 or FP4 with one scale per 32 values of a row.
    fn row32(
        &mut self,
        stem: &str,
        (rows, cols): (usize, usize),
        rng: &mut Rng,
        amplitude: f32,
        fp4: bool,
    ) {
        let top = if fp4 { 6.0 } else { 448.0 };
        let mut codes = Vec::new();
        let mut scales = Vec::new();
        for _ in 0..rows {
            let row: Vec<f32> = (0..cols).map(|_| rng.unit() * amplitude).collect();
            let mut row_codes = Vec::new();
            for block in row.chunks_exact(32) {
                let byte = scale_exponent(block.iter().fold(0.0_f32, |a, v| a.max(v.abs())), top);
                scales.push(byte);
                let s = decode_f8_e8m0fnu(byte);
                for v in block {
                    row_codes.push(if fp4 {
                        encode_fp4(v / s).unwrap_or(0)
                    } else {
                        encode_fp8(v / s).unwrap_or(0)
                    });
                }
            }
            if fp4 {
                codes.extend(row_codes.chunks_exact(2).map(|p| p[0] | (p[1] << 4)));
            } else {
                codes.extend(row_codes);
            }
        }
        let (dtype, width) = if fp4 {
            (V41TensorDtype::I8, cols / 2)
        } else {
            (V41TensorDtype::F8E4M3, cols)
        };
        self.insert(format!("{stem}.weight"), dtype, vec![rows, width], codes);
        self.insert(
            format!("{stem}.scale"),
            V41TensorDtype::F8E8M0,
            vec![rows, cols / 32],
            scales,
        );
    }
}

/// The model's weights, generated from `seed`.
#[must_use]
pub fn store(seed: u64) -> Store {
    let mut rng = Rng(seed);
    let (h, s, e) = (HIDDEN, STREAMS, EXPERT_WIDTH);
    let mut store = Store {
        tensors: BTreeMap::new(),
    };
    let layout = layout();
    let near_one =
        |rng: &mut Rng, n: usize| -> Vec<f32> { (0..n).map(|_| 1.0 + 0.1 * rng.unit()).collect() };
    let amp = |cols: usize| 1.7 / (cols as f32).sqrt();
    for layer in 0..40_usize {
        let name = |part: &str| format!("layers.{layer}.{part}");
        for part in ["attn", "ffn"] {
            let fnv: Vec<f32> = (0..24 * s * h).map(|_| rng.unit() * 0.05).collect();
            store.f32s(name(&format!("hc_{part}_fn")), vec![24, s * h], &fnv);
            store.f32s(name(&format!("hc_{part}_scale")), vec![3], &[0.5, 0.5, 0.5]);
            let base: Vec<f32> = (0..24)
                .map(|i| if i < 4 { -1.0 } else { 0.3 * rng.unit() })
                .collect();
            store.f32s(name(&format!("hc_{part}_base")), vec![24], &base);
        }
        let v = near_one(&mut rng, h);
        store.bf16s(name("attn_norm.weight"), vec![h], &v);
        let v = near_one(&mut rng, h);
        store.bf16s(name("ffn_norm.weight"), vec![h], &v);
        store.fp8(&name("attn.wq_a"), Q_RANK, h, &mut rng, amp(h));
        let v = near_one(&mut rng, Q_RANK);
        store.bf16s(name("attn.q_norm.weight"), vec![Q_RANK], &v);
        store.fp8(
            &name("attn.wq_b"),
            HEADS * HEAD_DIM,
            Q_RANK,
            &mut rng,
            amp(Q_RANK),
        );
        store.fp8(&name("attn.wkv"), HEAD_DIM, h, &mut rng, amp(h));
        let v = near_one(&mut rng, HEAD_DIM);
        store.bf16s(name("attn.kv_norm.weight"), vec![HEAD_DIM], &v);
        let sinks: Vec<f32> = (0..HEADS).map(|_| rng.unit()).collect();
        store.f32s(name("attn.attn_sink"), vec![HEADS], &sinks);
        let wo_cols = HEADS * HEAD_DIM / O_GROUPS;
        store.fp8(
            &name("attn.wo_a"),
            O_GROUPS * O_RANK,
            wo_cols,
            &mut rng,
            amp(wo_cols),
        );
        store.fp8(
            &name("attn.wo_b"),
            h,
            O_GROUPS * O_RANK,
            &mut rng,
            amp(O_GROUPS * O_RANK),
        );
        if [2, 8, 14, 20].contains(&layer) {
            let w: Vec<f32> = (0..HEAD_DIM * h).map(|_| rng.unit() * amp(h)).collect();
            store.bf16s(name("attn.compressor.wkv.weight"), vec![HEAD_DIM, h], &w);
            if layer < 20 {
                let w: Vec<f32> = (0..HEAD_DIM * h).map(|_| rng.unit() * amp(h)).collect();
                store.bf16s(name("attn.compressor.wgate.weight"), vec![HEAD_DIM, h], &w);
            }
            let v = near_one(&mut rng, HEAD_DIM);
            store.bf16s(name("attn.compressor.norm.weight"), vec![HEAD_DIM], &v);
            let w: Vec<f32> = (0..INDEX_DIM * HEAD_DIM)
                .map(|_| rng.unit() * amp(HEAD_DIM))
                .collect();
            store.bf16s(
                name("attn.indexer.wk.weight"),
                vec![INDEX_DIM, HEAD_DIM],
                &w,
            );
            let v = near_one(&mut rng, INDEX_DIM);
            store.bf16s(name("attn.indexer.k_norm.weight"), vec![INDEX_DIM], &v);
        }
        if [2, 8, 14, 20, 24, 28, 32, 36].contains(&layer) {
            store.fp8(
                &name("attn.indexer.wq_b"),
                INDEX_HEADS * INDEX_DIM,
                Q_RANK,
                &mut rng,
                amp(Q_RANK),
            );
            let w: Vec<f32> = (0..INDEX_HEADS * h).map(|_| rng.unit() * amp(h)).collect();
            store.bf16s(
                name("attn.indexer.weights_proj.weight"),
                vec![INDEX_HEADS, h],
                &w,
            );
        }
        let gate: Vec<f32> = (0..EXPERTS * h)
            .map(|_| rng.unit() * 0.5 * amp(h))
            .collect();
        store.bf16s(name("ffn.gate.weight"), vec![EXPERTS, h], &gate);
        // Biases spread the experts so router near ties stay rare.
        let bias: Vec<f32> = (0..EXPERTS).map(|i| ((i * 7) % 16) as f32 * 0.02).collect();
        store.f32s(name("ffn.gate.bias"), vec![EXPERTS], &bias);
        for i in 0..EXPERTS {
            let stem = name(&format!("ffn.experts.{i}"));
            store.row32(&format!("{stem}.w1"), (e, h), &mut rng, amp(h), true);
            store.row32(&format!("{stem}.w2"), (h, e), &mut rng, amp(e), true);
            store.row32(&format!("{stem}.w3"), (e, h), &mut rng, amp(h), true);
        }
        store.fp8(&name("ffn.shared_experts.w1"), e, h, &mut rng, amp(h));
        store.fp8(&name("ffn.shared_experts.w2"), h, e, &mut rng, amp(e));
        store.fp8(&name("ffn.shared_experts.w3"), e, h, &mut rng, amp(h));
        if let Some(l) = layout.layers.iter().find(|l| l.layer_id as usize == layer) {
            let q = near_one(&mut rng, s * h);
            store.bf16s(name("engram.q_weight"), vec![s, h], &q);
            let k = near_one(&mut rng, s * h);
            store.bf16s(name("engram.k_weight"), vec![s, h], &k);
            let k = 24 * ENGRAM_WIDTH;
            store.fp8(&name("engram.wkv"), s * h + h, k, &mut rng, amp(k));
            store.row32(
                &name("engram.embed"),
                (l.table_rows as usize, ENGRAM_WIDTH),
                &mut rng,
                1.0,
                false,
            );
        }
    }
    let embed: Vec<f32> = (0..VOCAB * h).map(|_| rng.unit()).collect();
    store.bf16s("embed.weight".into(), vec![VOCAB, h], &embed);
    let head: Vec<f32> = (0..VOCAB * h).map(|_| rng.unit() * 0.2).collect();
    store.bf16s("head.weight".into(), vec![VOCAB, h], &head);
    let v = near_one(&mut rng, h);
    store.bf16s("norm.weight".into(), vec![h], &v);
    store
}

/// The Engram tables of `layers`, in page-locked host memory.
///
/// # Errors
///
/// Any allocation or copy failure.
pub fn engram_tables(
    backend: &mut CudaBackend,
    store: &Store,
    layers: &Range<usize>,
) -> Result<Vec<V41EngramHostTable>, RuntimeError> {
    let mut out = Vec::new();
    for l in layout().layers {
        let layer = l.layer_id as usize;
        if !layers.contains(&layer) {
            continue;
        }
        let codes = store.tensor(&format!("layers.{layer}.engram.embed.weight"))?;
        let scales = store.tensor(&format!("layers.{layer}.engram.embed.scale"))?;
        let mut c = backend.host_buffer(codes.bytes.len())?;
        c.write_bytes(&codes.bytes)?;
        let mut s = backend.host_buffer(scales.bytes.len())?;
        s.write_bytes(&scales.bytes)?;
        out.push(V41EngramHostTable {
            layer,
            codes: c,
            scales: s,
            rows: l.table_rows,
        });
    }
    Ok(out)
}
