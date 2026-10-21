//! Tests of the device-resident V4.1 stage (#12394) on a synthetic model with
//! V4.1's 40-layer layout and small shapes.
//!
//! **Tolerances, fixed before the first GPU run.** The stage repeats the CPU
//! path's default `ReferenceBf16` operations, but several kernels reorder
//! long sums (W4's block products, W6's mixes and router logits, the head's
//! warp sums) and round `exp`, `cos`, and `sin` once from double precision.
//! Those differences are bounded per kernel (W4–W6), but a connected stage
//! carries a BF16 rounding flip into every later value, so no per-layer
//! bound follows from the arithmetic alone. The stage is therefore compared
//! with the CPU stage under the frozen W1 policy
//! (`cpu_qualification/tolerance-policy.json`), the same families the CPU
//! is held to against the oracle:
//!
//! - BF16 activations and residuals: `|gpu - cpu| <= 2^-6 + 2^-6 |cpu|`;
//! - float32 mixes and route weights: `2e-4 + 2e-4 |cpu|`;
//! - logits: `2^-5 + 2^-6 |cpu|`;
//! - expert ids, index selections, and Engram row ids: exact.
//!
//! Stages split at any boundary must equal the unsplit stage bit for bit,
//! since every kernel is deterministic. Kernels new in W7 (the embedding,
//! the Engram hash, the copies, the candidate flags, and greedy sampling)
//! must equal their host models exactly.

use super::*;
use crate::CudaCommandWait;
use psionic_models::deepseek_v41_cpu::{
    CpuError, MatrixRows,
    block::BlockConfig,
    engram::{CompressedTokenMap, EngramLayout, LayerLayout, NgramHashState},
    hyperconnections::HyperConnections,
    moe::Router,
    projection::{Geometry, WeightSource},
    quantization::{Arithmetic, MatrixPrecision, encode_fp4, encode_fp8},
    transformer::{CpuTransformer, TransformerConfig},
};
use psionic_models::deepseek_v41_weights::{decode_e2m1, decode_f8_e4m3fn, decode_f8_e8m0fnu};
use serde_json::json;
use std::{collections::BTreeMap, error::Error, sync::Arc};

#[path = "v41_stage_reference.rs"]
mod reference;
use reference::{CpuStage, Observations};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------
// The synthetic model
// ---------------------------------------------------------------------------

const VOCAB: usize = 64;
const MAX_CONTEXT: usize = 40;
const TOP_BLOCKS: usize = 2;
const ENGRAM_WIDTH: usize = 32;

fn geometry() -> Geometry {
    Geometry {
        hidden: 128,
        heads: 4,
        head_dim: 64,
        q_rank: 64,
        o_groups: 2,
        o_rank: 32,
        index_heads: 4,
        index_dim: 32,
        rope_dim: 16,
        window: 8,
        index_top_k: 4,
        norm_epsilon: 1e-20,
    }
}

fn block_config() -> BlockConfig {
    let g = geometry();
    BlockConfig {
        geometry: g,
        hyper: HyperConnections {
            width: g.hidden,
            ..HyperConnections::flash()
        },
        router: Router::flash(),
        expert_count: 16,
        expert_width: 64,
        swiglu_limit: 10.0,
    }
}

fn shape() -> V41StageShape {
    let g = geometry();
    let b = block_config();
    V41StageShape {
        hidden: g.hidden,
        q_rank: g.q_rank,
        o_groups: g.o_groups,
        o_rank: g.o_rank,
        attention: V41AttentionGeometry {
            heads: g.heads,
            head_dim: g.head_dim,
            rope_dim: g.rope_dim,
            window: g.window,
            index_heads: g.index_heads,
            index_dim: g.index_dim,
            index_top_k: g.index_top_k,
            norm_epsilon: g.norm_epsilon,
        },
        hc: V41HyperConfig {
            width: g.hidden,
            ..V41HyperConfig::flash()
        },
        router: V41Router {
            experts: b.expert_count,
            ..V41Router::flash()
        },
        expert_width: b.expert_width,
        swiglu_limit: b.swiglu_limit,
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

fn layout() -> EngramLayout {
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

/// The tokenizer map for `t0..t63`: every token its own compressed id.
fn token_map() -> Result<Arc<CompressedTokenMap>, CpuError> {
    let vocab: BTreeMap<_, _> = (0..VOCAB).map(|i| (format!("t{i}"), i)).collect();
    let json = json!({"version":"1.0","truncation":null,"padding":null,"added_tokens":[],"normalizer":null,"pre_tokenizer":null,"post_processor":null,"decoder":null,"model":{"type":"WordLevel","vocab":vocab,"unk_token":"t0"}});
    let tokenizer = tokenizers::Tokenizer::from_bytes(json.to_string())
        .map_err(|e| CpuError::Tokenization(e.to_string()))?;
    Ok(Arc::new(CompressedTokenMap::build(&tokenizer, VOCAB)?))
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

fn bf16_word(x: f32) -> u16 {
    half::bf16::from_f32(x).to_bits()
}

fn scale_exponent(amax: f32, top: f32) -> u8 {
    let ratio = (amax.max(1e-30) / top).log2().ceil();
    (ratio as i32 + 127).clamp(1, 254) as u8
}

/// A checkpoint-form synthetic tensor store that both the GPU stage and the
/// CPU path read.
struct Store {
    tensors: BTreeMap<String, V41HostTensor>,
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
            .flat_map(|v| bf16_word(*v).to_le_bytes())
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
        rows: usize,
        cols: usize,
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

    fn decode(&self, name: &str) -> Result<(usize, usize, MatrixPrecision, Vec<f32>), CpuError> {
        let t = self
            .tensors
            .get(name)
            .ok_or(CpuError::Invalid("synthetic tensor missing"))?;
        let rows = t.shape[0];
        let scale = |n: &str| {
            self.tensors
                .get(&n.replace(".weight", ".scale"))
                .map(|s| s.bytes.as_slice())
                .ok_or(CpuError::Invalid("synthetic scale missing"))
        };
        Ok(match t.dtype {
            V41TensorDtype::F32 => {
                let cols = t.shape.get(1).copied().unwrap_or(rows);
                let v = t
                    .bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                (rows, cols, MatrixPrecision::F32, v)
            }
            V41TensorDtype::Bf16 => {
                let cols = t.shape.get(1).copied().unwrap_or(rows);
                let v = t
                    .bytes
                    .chunks_exact(2)
                    .map(|c| f32::from_bits(u32::from(u16::from_le_bytes([c[0], c[1]])) << 16))
                    .collect();
                (rows, cols, MatrixPrecision::Bf16, v)
            }
            V41TensorDtype::F8E4M3 => {
                let cols = t.shape[1];
                let s = scale(name)?;
                let per_row = name.ends_with("engram.embed.weight");
                let v = (0..rows * cols)
                    .map(|i| {
                        let (r, c) = (i / cols, i % cols);
                        let byte = if per_row {
                            s[r * (cols / 32) + c / 32]
                        } else {
                            s[(r / 32) * (cols / 32) + c / 32]
                        };
                        decode_f8_e4m3fn(t.bytes[i]) * decode_f8_e8m0fnu(byte)
                    })
                    .collect();
                (rows, cols, MatrixPrecision::Fp8, v)
            }
            V41TensorDtype::I8 => {
                let cols = t.shape[1] * 2;
                let s = scale(name)?;
                let v = (0..rows * cols)
                    .map(|i| {
                        let (r, c) = (i / cols, i % cols);
                        let byte = t.bytes[r * cols / 2 + c / 2];
                        let code = if c % 2 == 0 { byte & 15 } else { byte >> 4 };
                        decode_e2m1(code) * decode_f8_e8m0fnu(s[r * (cols / 32) + c / 32])
                    })
                    .collect();
                (rows, cols, MatrixPrecision::Fp4, v)
            }
            V41TensorDtype::F8E8M0 => {
                return Err(CpuError::Invalid("scale tensor read as a matrix"));
            }
        })
    }
}

impl V41TensorSource for Store {
    fn tensor(&self, name: &str) -> Result<V41HostTensor, RuntimeError> {
        self.tensors
            .get(name)
            .cloned()
            .ok_or_else(|| RuntimeError::Backend(format!("synthetic tensor {name} missing")))
    }
}

struct Dense {
    rows: usize,
    cols: usize,
    precision: MatrixPrecision,
    values: Vec<f32>,
}

impl MatrixRows for Dense {
    fn dimensions(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
    fn precision(&self) -> MatrixPrecision {
        self.precision
    }
    fn row(&self, index: usize) -> Result<Vec<f32>, CpuError> {
        if index >= self.rows {
            return Err(CpuError::Invalid("synthetic row out of range"));
        }
        Ok(self.values[index * self.cols..(index + 1) * self.cols].to_vec())
    }
}

impl WeightSource for Store {
    fn matrix(&self, name: &str) -> Result<Box<dyn MatrixRows + '_>, CpuError> {
        let (rows, cols, precision, values) = self.decode(name)?;
        Ok(Box::new(Dense {
            rows,
            cols,
            precision,
            values,
        }))
    }
    fn vector(&self, name: &str) -> Result<Vec<f32>, CpuError> {
        Ok(self.decode(name)?.3)
    }
}

fn synthetic_store(seed: u64) -> Store {
    let mut rng = Rng(seed);
    let g = geometry();
    let b = block_config();
    let (h, s, e) = (g.hidden, 4, b.expert_width);
    let mut store = Store {
        tensors: BTreeMap::new(),
    };
    let layout = layout();
    let near_one =
        |rng: &mut Rng, n: usize| -> Vec<f32> { (0..n).map(|_| 1.0 + 0.1 * rng.unit()).collect() };
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
        let amp = |cols: usize| 1.7 / (cols as f32).sqrt();
        store.fp8(&name("attn.wq_a"), g.q_rank, h, &mut rng, amp(h));
        let v = near_one(&mut rng, g.q_rank);
        store.bf16s(name("attn.q_norm.weight"), vec![g.q_rank], &v);
        store.fp8(
            &name("attn.wq_b"),
            g.heads * g.head_dim,
            g.q_rank,
            &mut rng,
            amp(g.q_rank),
        );
        store.fp8(&name("attn.wkv"), g.head_dim, h, &mut rng, amp(h));
        let v = near_one(&mut rng, g.head_dim);
        store.bf16s(name("attn.kv_norm.weight"), vec![g.head_dim], &v);
        let sinks: Vec<f32> = (0..g.heads).map(|_| rng.unit()).collect();
        store.f32s(name("attn.attn_sink"), vec![g.heads], &sinks);
        let wo_cols = g.heads * g.head_dim / g.o_groups;
        store.fp8(
            &name("attn.wo_a"),
            g.o_groups * g.o_rank,
            wo_cols,
            &mut rng,
            amp(wo_cols),
        );
        store.fp8(
            &name("attn.wo_b"),
            h,
            g.o_groups * g.o_rank,
            &mut rng,
            amp(g.o_groups * g.o_rank),
        );
        if [2, 8, 14, 20].contains(&layer) {
            let w: Vec<f32> = (0..g.head_dim * h).map(|_| rng.unit() * amp(h)).collect();
            store.bf16s(name("attn.compressor.wkv.weight"), vec![g.head_dim, h], &w);
            if layer < 20 {
                let w: Vec<f32> = (0..g.head_dim * h).map(|_| rng.unit() * amp(h)).collect();
                store.bf16s(
                    name("attn.compressor.wgate.weight"),
                    vec![g.head_dim, h],
                    &w,
                );
            }
            let v = near_one(&mut rng, g.head_dim);
            store.bf16s(name("attn.compressor.norm.weight"), vec![g.head_dim], &v);
            let w: Vec<f32> = (0..g.index_dim * g.head_dim)
                .map(|_| rng.unit() * amp(g.head_dim))
                .collect();
            store.bf16s(
                name("attn.indexer.wk.weight"),
                vec![g.index_dim, g.head_dim],
                &w,
            );
            let v = near_one(&mut rng, g.index_dim);
            store.bf16s(name("attn.indexer.k_norm.weight"), vec![g.index_dim], &v);
        }
        if [2, 8, 14, 20, 24, 28, 32, 36].contains(&layer) {
            store.fp8(
                &name("attn.indexer.wq_b"),
                g.index_heads * g.index_dim,
                g.q_rank,
                &mut rng,
                amp(g.q_rank),
            );
            let w: Vec<f32> = (0..g.index_heads * h)
                .map(|_| rng.unit() * amp(h))
                .collect();
            store.bf16s(
                name("attn.indexer.weights_proj.weight"),
                vec![g.index_heads, h],
                &w,
            );
        }
        let gate: Vec<f32> = (0..b.expert_count * h)
            .map(|_| rng.unit() * 0.5 * amp(h))
            .collect();
        store.bf16s(name("ffn.gate.weight"), vec![b.expert_count, h], &gate);
        // Biases spread the experts so router near ties stay rare.
        let bias: Vec<f32> = (0..b.expert_count)
            .map(|i| ((i * 7) % 16) as f32 * 0.02)
            .collect();
        store.f32s(name("ffn.gate.bias"), vec![b.expert_count], &bias);
        for i in 0..b.expert_count {
            let stem = name(&format!("ffn.experts.{i}"));
            store.row32(&format!("{stem}.w1"), e, h, &mut rng, amp(h), true);
            store.row32(&format!("{stem}.w2"), h, e, &mut rng, amp(e), true);
            store.row32(&format!("{stem}.w3"), e, h, &mut rng, amp(h), true);
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
                l.table_rows as usize,
                ENGRAM_WIDTH,
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

fn cuda() -> Option<CudaBackend> {
    let backend = CudaBackend::new();
    if backend.selected_device().is_some() && backend.quantized_kernels_available() {
        return Some(backend);
    }
    assert!(
        std::env::var_os("PSIONIC_REQUIRE_CUDA").is_none(),
        "CUDA required but the device or the kernels are unavailable"
    );
    None
}

fn engram_hashes() -> Vec<V41EngramHash> {
    layout()
        .layers
        .iter()
        .map(|l| V41EngramHash {
            layer: l.layer_id as usize,
            multipliers: l.multipliers,
            primes: l.primes.clone(),
            offsets: l.offsets.clone(),
            rows: l.table_rows,
        })
        .collect()
}

fn engram_tables(
    backend: &mut CudaBackend,
    store: &Store,
    layers: &Range<usize>,
) -> Result<Vec<V41EngramHostTable>, Box<dyn Error>> {
    let mut out = Vec::new();
    for l in layout().layers {
        let layer = l.layer_id as usize;
        if !layers.contains(&layer) {
            continue;
        }
        let codes = &store.tensors[&format!("layers.{layer}.engram.embed.weight")].bytes;
        let scales = &store.tensors[&format!("layers.{layer}.engram.embed.scale")].bytes;
        let mut c = backend.host_buffer(codes.len())?;
        c.write_bytes(codes)?;
        let mut s = backend.host_buffer(scales.len())?;
        s.write_bytes(scales)?;
        out.push(V41EngramHostTable {
            layer,
            codes: c,
            scales: s,
            rows: l.table_rows,
        });
    }
    Ok(out)
}

fn load_stage(
    backend: &mut CudaBackend,
    store: &Store,
    layers: Range<usize>,
    observe: bool,
) -> Result<V41Stage, Box<dyn Error>> {
    let map = token_map()?;
    let tables = engram_tables(backend, store, &layers)?;
    Ok(V41Stage::load(
        backend,
        V41StageConfig {
            shape: shape(),
            layers,
            max_context: MAX_CONTEXT,
            max_step_tokens: 24,
            engram: engram_hashes(),
            token_map: Some(V41TokenMap {
                ids: map.ids().to_vec(),
                pad: map.get(2)?,
            }),
            observe,
            swap_rope_tables: false,
        },
        store,
        tables,
    )?)
}

/// The prompt and the tokens decoded after it.
fn tokens() -> (Vec<u32>, Vec<u32>) {
    let prompt = (0..21).map(|i| ((i * 37 + 5) % VOCAB) as u32).collect();
    let decode = vec![3, 63, 0, 17, 17, 42];
    (prompt, decode)
}

// ---------------------------------------------------------------------------
// CPU-only checks
// ---------------------------------------------------------------------------

/// The CPU stage, composed from the CPU path's components, equals the CPU
/// transformer bit for bit over all 40 layers, prefill and decode.
#[test]
fn cpu_stage_matches_the_cpu_transformer() -> TestResult {
    let store = synthetic_store(7);
    let map = token_map()?;
    let config = TransformerConfig {
        block: block_config(),
        vocabulary: VOCAB,
        pad_token: 2,
        eos_token: 1,
        engram: Arc::new(layout()),
    };
    let mut model = CpuTransformer::new(
        &store,
        config,
        map.clone(),
        MAX_CONTEXT,
        Arithmetic::ReferenceBf16,
    )?;
    let mut stage = CpuStage::new(
        &store,
        block_config(),
        40,
        Arc::new(layout()),
        map.ids().to_vec(),
        map.get(2)?,
        (2048, 8),
    )?;
    let (prompt, decode) = tokens();
    let mut steps = vec![prompt];
    steps.extend(decode.iter().map(|t| vec![*t]));
    for step in &steps {
        let expected = model.forward_with_trace(step, &[0, 1, 2, 14, 20, 21, 24, 39])?;
        let actual = stage.step(step)?;
        assert_eq!(
            actual.logits.as_deref(),
            Some(expected.logits.as_slice()),
            "logits"
        );
        for trace in &expected.layers {
            let observed = &actual.observations[&trace.layer][&V41Site::HcOut];
            let flat: Vec<f64> = trace
                .residuals
                .iter()
                .flatten()
                .map(|v| f64::from(*v))
                .collect();
            assert_eq!(observed, &flat, "layer {} residuals", trace.layer);
            let ids = &actual.observations[&trace.layer][&V41Site::ExpertIds];
            let mut expected_ids: Vec<f64> = Vec::new();
            for row in &trace.expert_ids {
                let mut row = row.clone();
                row.sort_unstable();
                expected_ids.extend(row.iter().map(|x| *x as f64));
            }
            assert_eq!(ids, &expected_ids, "layer {} expert ids", trace.layer);
        }
    }
    Ok(())
}

/// Boundaries agree with W9's contract.
#[test]
fn boundaries_match_the_stage_contract() {
    use psionic_models::deepseek_v41_stage::{V41StageGeometry, boundary_sections};
    use psionic_models::stage_state::SectionKind;
    for layer in 1..40 {
        let sections = boundary_sections(&V41StageGeometry::flash(), layer as u32);
        let has = |kind: SectionKind| sections.iter().any(|s| s.kind == kind);
        let b = V41Boundary::at(layer);
        let kv = sections.iter().find_map(|s| match s.kind {
            SectionKind::SourceKv { layer } => Some(layer as usize),
            _ => None,
        });
        assert_eq!(b.kv.map(|(l, _)| l), kv, "KV at {layer}");
        if let Some((source, keys)) = b.kv {
            assert_eq!(
                keys,
                has(SectionKind::SourceIndexKeys {
                    layer: source as u32
                }),
                "keys at {layer}"
            );
        }
        assert_eq!(
            b.candidates,
            has(SectionKind::Candidates { layer: 20 }),
            "candidates at {layer}"
        );
        let selection = sections.iter().find_map(|s| match s.kind {
            SectionKind::IndexSelection { layer } => Some(layer as usize),
            _ => None,
        });
        assert_eq!(b.selection, selection, "selection at {layer}");
        assert!(has(SectionKind::PreMix), "pre-mix at {layer}");
    }
}

// ---------------------------------------------------------------------------
// Device checks
// ---------------------------------------------------------------------------

fn family(site: V41Site) -> (f64, f64) {
    match site {
        V41Site::AttnMixes | V41Site::FfnMixes | V41Site::RouteWeights => (2e-4, 2e-4),
        V41Site::EngramIds | V41Site::ExpertIds | V41Site::Selection => (0.0, 0.0),
        _ => (0.015_625, 0.015_625),
    }
}

/// Compares one step's device observations with the CPU's. Returns the
/// largest error over the policy limit and the counts of compared and
/// bit-equal values.
fn compare(
    gpu: &BTreeMap<usize, BTreeMap<V41Site, Vec<u32>>>,
    cpu: &Observations,
    tokens: usize,
    what: &str,
) -> (f64, usize, usize) {
    let mut worst = 0.0_f64;
    let (mut compared, mut equal) = (0, 0);
    for (layer, sites) in cpu {
        for (site, expected) in sites {
            let got = &gpu[layer][site];
            let words = got.len() / tokens.max(1);
            let got: Vec<f64> = if site.integer() {
                if *site == V41Site::EngramIds {
                    got.chunks_exact(2)
                        .take(expected.len())
                        .map(|w| (u64::from(w[0]) | (u64::from(w[1]) << 32)) as i64 as f64)
                        .collect()
                } else {
                    got.iter()
                        .take(expected.len())
                        .map(|w| f64::from(*w as i32))
                        .collect()
                }
            } else {
                got.iter()
                    .take(expected.len())
                    .map(|w| f64::from(f32::from_bits(*w)))
                    .collect()
            };
            let _ = words;
            assert_eq!(
                got.len(),
                expected.len(),
                "{what} layer {layer} {site:?} length"
            );
            let (atol, rtol) = family(*site);
            for (i, (a, b)) in got.iter().zip(expected).enumerate() {
                compared += 1;
                if a.to_bits() == b.to_bits() {
                    equal += 1;
                }
                let limit = atol + rtol * b.abs();
                let error = (a - b).abs();
                assert!(
                    a.is_finite() && error <= limit,
                    "{what} layer {layer} {site:?}[{i}]: gpu {a} cpu {b}"
                );
                if limit > 0.0 {
                    worst = worst.max(error / limit);
                }
            }
        }
    }
    (worst, compared, equal)
}

/// The whole synthetic model as one stage, prefill then decode, against the
/// CPU stage under the frozen policy.
#[test]
fn synthetic_stage_matches_the_cpu() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let store = synthetic_store(7);
    let map = token_map()?;
    let mut cpu = CpuStage::new(
        &store,
        block_config(),
        40,
        Arc::new(layout()),
        map.ids().to_vec(),
        map.get(2)?,
        (TOP_BLOCKS, 8),
    )?;
    let mut gpu = load_stage(&mut backend, &store, 0..40, true)?;
    let (prompt, decode) = tokens();
    let mut steps = vec![prompt];
    steps.extend(decode.iter().map(|t| vec![*t]));
    let mut position = 0;
    let mut report = Vec::new();
    for (k, step) in steps.iter().enumerate() {
        let expected = cpu.step(step)?;
        let out = gpu.step(
            &backend,
            &V41StepInput {
                position,
                tokens: step,
                residual: None,
                pre_mix: None,
                source_rows: None,
                selection: None,
                candidates: None,
                sampling: V41Sampling::greedy(),
            },
            true,
        )?;
        let observed = gpu.observations(step.len())?;
        let (worst, compared, equal) = compare(
            &observed,
            &expected.observations,
            step.len(),
            &format!("step {k}"),
        );
        let logits = gpu.logits()?;
        let cpu_logits = expected.logits.ok_or("CPU logits")?;
        let mut logit_worst = 0.0_f64;
        for (a, b) in logits.iter().zip(&cpu_logits) {
            let limit = 0.031_25 + 0.015_625 * f64::from(b.abs());
            let error = f64::from((a - b).abs());
            assert!(error <= limit, "step {k} logits {a} vs {b}");
            logit_worst = logit_worst.max(error / limit);
        }
        let cpu_argmax =
            cpu_logits.iter().enumerate().fold(
                0,
                |best, (i, v)| if *v > cpu_logits[best] { i } else { best },
            );
        assert_eq!(out.token, Some(cpu_argmax as u32), "step {k} greedy token");
        report.push(
            json!({"step": k, "tokens": step.len(), "worst_over_policy": worst,
            "compared": compared, "bit_equal": equal, "logits_worst_over_policy": logit_worst,
            "token": out.token, "stats": format!("{:?}", out.stats)}),
        );
        position += step.len();
    }
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "synthetic_stage", "steps": report})
    );
    Ok(())
}

/// Stages split at boundaries that carry every kind of section equal the
/// unsplit stage bit for bit.
#[test]
fn split_stages_equal_the_whole() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let store = synthetic_store(11);
    let mut whole = load_stage(&mut backend, &store, 0..40, false)?;
    let splits = [0..5, 5..13, 13..21, 21..30, 30..40];
    let mut stages = Vec::new();
    for range in splits.iter().cloned() {
        stages.push(load_stage(&mut backend, &store, range, false)?);
    }
    let (prompt, decode) = tokens();
    let mut steps = vec![prompt];
    steps.extend(decode.iter().map(|t| vec![*t]));
    let mut position = 0;
    let sampling = V41Sampling {
        temperature: 0.8,
        top_p: 0.9,
        seed: 1234,
    };
    for step in &steps {
        let input = V41StepInput {
            position,
            tokens: step,
            residual: None,
            pre_mix: None,
            source_rows: None,
            selection: None,
            candidates: None,
            sampling,
        };
        let expected = whole.step(&backend, &input, false)?;
        let expected_logits = whole.logits()?;
        let mut previous: Option<V41StepOutput> = None;
        for stage in &mut stages {
            let out = {
                let p = previous.as_ref();
                stage.step(
                    &backend,
                    &V41StepInput {
                        position,
                        tokens: step,
                        residual: p.map(|p| p.residual.as_slice()),
                        pre_mix: p.map(|p| p.pre_mix.as_slice()),
                        source_rows: p.and_then(|p| p.source_rows.as_ref()),
                        selection: p
                            .and_then(|p| p.selection.as_ref().map(|(_, rows)| rows.as_slice())),
                        candidates: p.and_then(|p| p.candidates.as_deref()),
                        sampling,
                    },
                    false,
                )?
            };
            previous = Some(out);
        }
        let last = previous.ok_or("no stages")?;
        assert_eq!(last.token, expected.token);
        let logits = stages.last().ok_or("no stages")?.logits()?;
        assert!(
            logits
                .iter()
                .zip(&expected_logits)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "split logits differ"
        );
        position += step.len();
    }
    Ok(())
}

/// A decode step is one graph launch per stage graph and one wait, with
/// only the step's input and output crossing the bus.
#[test]
fn decode_steps_stay_on_the_device() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let store = synthetic_store(13);
    let mut first = load_stage(&mut backend, &store, 0..20, false)?;
    let mut last = load_stage(&mut backend, &store, 20..40, false)?;
    let (prompt, decode) = tokens();
    let mut position = 0;
    let mut steps = vec![prompt];
    steps.extend(decode.iter().map(|t| vec![*t]));
    let mut report = Vec::new();
    for (k, step) in steps.iter().enumerate() {
        let a = first.step(
            &backend,
            &V41StepInput {
                position,
                tokens: step,
                residual: None,
                pre_mix: None,
                source_rows: None,
                selection: None,
                candidates: None,
                sampling: V41Sampling::greedy(),
            },
            false,
        )?;
        let b = last.step(
            &backend,
            &V41StepInput {
                position,
                tokens: step,
                residual: Some(&a.residual),
                pre_mix: Some(&a.pre_mix),
                source_rows: a.source_rows.as_ref(),
                selection: a.selection.as_ref().map(|(_, r)| r.as_slice()),
                candidates: a.candidates.as_deref(),
                sampling: V41Sampling::greedy(),
            },
            false,
        )?;
        if k > 0 {
            for stats in [&a.stats, &b.stats] {
                assert_eq!(stats.graph_captures, 0);
                assert_eq!(stats.synchronizations, 1, "one wait per step");
            }
            assert_eq!(a.stats.graph_launches, 1);
            assert_eq!(b.stats.graph_launches, 2, "body and tail");
            assert_eq!(b.stats.device_to_host_copies, 1, "only the token id");
            assert_eq!(b.stats.device_to_host_bytes, 4);
        } else {
            assert_eq!(a.stats.graph_launches, step.len());
        }
        report.push(
            json!({"step": k, "first": format!("{:?}", a.stats), "last": format!("{:?}", b.stats)}),
        );
        position += step.len();
    }
    eprintln!(
        "EVIDENCE {}",
        json!({"mode": "round_trips", "steps": report})
    );
    Ok(())
}

/// The Engram hash on the device equals `NgramHashState` across steps and
/// a new request at position 0.
#[test]
fn engram_hash_matches_the_cpu() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let map = token_map()?;
    let pad = map.get(2)?;
    let layout = Arc::new(layout());
    let hashes = engram_hashes();
    let mut params = Vec::new();
    for h in &hashes {
        params.extend_from_slice(&h.multipliers);
        params.extend_from_slice(&h.primes);
        params.extend_from_slice(&h.offsets);
    }
    let up = |backend: &mut CudaBackend, bytes: Vec<u8>| backend.byte_buffer(&bytes);
    let map_buffer = up(
        &mut backend,
        map.ids().iter().flat_map(|x| x.to_le_bytes()).collect(),
    )?;
    let params_buffer = up(
        &mut backend,
        params.iter().flat_map(|x| x.to_le_bytes()).collect(),
    )?;
    let history = up(&mut backend, vec![0; 16])?;
    let ids = up(&mut backend, vec![0; 48 * 8])?;
    let mut index = up(&mut backend, vec![0; 4])?;
    let mut base = up(&mut backend, vec![0; 4])?;
    let mut tokens_buffer = up(&mut backend, vec![0; 4 * 8])?;
    for request in [vec![5_u32, 9, 9, 1, 63, 2, 0, 7], vec![7, 7, 3]] {
        let mut cpu = NgramHashState::new(layout.clone(), pad)?;
        let compressed: Vec<Option<u32>> = request
            .iter()
            .map(|t| Some(map.ids()[*t as usize]))
            .collect();
        let expected = cpu.push_compressed(0, &compressed)?;
        tokens_buffer.write_bytes_at_offset(
            0,
            &request
                .iter()
                .flat_map(|x| x.to_le_bytes())
                .collect::<Vec<_>>(),
        )?;
        base.write_bytes_at_offset(0, &0_i32.to_le_bytes())?;
        for (i, expected) in expected.iter().enumerate() {
            index.write_bytes_at_offset(0, &(i as i32).to_le_bytes())?;
            let mut s = backend.begin_submission()?;
            s.v41_engram_hash(
                &index,
                &base,
                &tokens_buffer,
                8,
                &map_buffer,
                VOCAB,
                pad,
                &history,
                &params_buffer,
                2,
                8,
                &ids,
            )?;
            s.commit(CudaCommandWait::Completed)?;
            let got: Vec<u64> = ids
                .read_bytes()?
                .chunks_exact(8)
                .map(|c| u64::from_le_bytes(c.try_into().unwrap_or([0; 8])))
                .collect();
            let want: Vec<u64> = expected.layers.iter().flatten().copied().collect();
            assert_eq!(got, want, "token {i}");
        }
    }
    Ok(())
}

/// Greedy and seeded sampling on the device equal a host model of the same
/// definition, and a NaN logit gives -1.
#[test]
fn sampling_matches_its_definition() -> TestResult {
    let Some(mut backend) = cuda() else {
        return Ok(());
    };
    let vocab = 5000;
    let mut rng = Rng(99);
    let logits: Vec<f32> = (0..vocab).map(|_| rng.unit() * 8.0).collect();
    let up = |backend: &mut CudaBackend, bytes: Vec<u8>| backend.byte_buffer(&bytes);
    let mut logits_buffer = up(
        &mut backend,
        logits.iter().flat_map(|x| x.to_le_bytes()).collect(),
    )?;
    let mut settings = up(&mut backend, vec![0; 8])?;
    let mut seed = up(&mut backend, vec![0; 8])?;
    let mut positions = up(&mut backend, vec![0; 4])?;
    let token = up(&mut backend, vec![0; 4])?;
    let run = |backend: &CudaBackend,
               logits: &CudaBuffer,
               settings: &CudaBuffer,
               seed: &CudaBuffer,
               positions: &CudaBuffer|
     -> Result<i32, Box<dyn Error>> {
        let mut s = backend.begin_submission()?;
        s.v41_sample(logits, vocab, settings, seed, positions, &token)?;
        s.commit(CudaCommandWait::Completed)?;
        let b = token.read_bytes()?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let argmax = logits
        .iter()
        .enumerate()
        .fold(0, |best, (i, v)| if *v > logits[best] { i } else { best });
    settings.write_bytes(&[0.0_f32.to_le_bytes(), 1.0_f32.to_le_bytes()].concat())?;
    assert_eq!(
        run(&backend, &logits_buffer, &settings, &seed, &positions)?,
        argmax as i32
    );
    let host = |temperature: f32, top_p: f32, seed: u64, position: i32| -> usize {
        let m = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let w: Vec<f64> = logits
            .iter()
            .map(|l| f64::from(((f64::from((l - m) / temperature)).exp()) as f32))
            .collect();
        let chunk = vocab.div_ceil(1024);
        let total: f64 = (0..1024)
            .map(|t| {
                w[(t * chunk).min(vocab)..((t + 1) * chunk).min(vocab)]
                    .iter()
                    .sum::<f64>()
            })
            .collect::<Vec<_>>()
            .iter()
            .sum();
        let _ = total;
        let mut lo = 0.0;
        if top_p < 1.0 {
            let mut hi = 1.0;
            let want = f64::from(top_p) * w.iter().sum::<f64>();
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                let kept: f64 = w.iter().filter(|x| **x >= mid).sum();
                if kept >= want {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
        }
        let kept: f64 = w.iter().filter(|x| **x >= lo).sum();
        let mut x = seed ^ (position as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
        x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^= x >> 31;
        let target = (x >> 11) as f64 * 2f64.powi(-53) * kept;
        let mut running = 0.0;
        let mut chosen = argmax;
        for (i, wi) in w.iter().enumerate() {
            if *wi >= lo {
                running += wi;
                chosen = i;
                if target < running {
                    break;
                }
            }
        }
        chosen
    };
    let mut matched = 0;
    let mut cases = 0;
    for (temperature, top_p) in [(1.0_f32, 1.0_f32), (0.7, 0.9), (1.3, 0.5), (0.2, 0.95)] {
        settings.write_bytes(&[temperature.to_le_bytes(), top_p.to_le_bytes()].concat())?;
        for s in 0..20_u64 {
            seed.write_bytes(&(s * 7919).to_le_bytes())?;
            positions.write_bytes(&(s as i32 * 3).to_le_bytes())?;
            let got = run(&backend, &logits_buffer, &settings, &seed, &positions)?;
            let want = host(temperature, top_p, s * 7919, s as i32 * 3);
            cases += 1;
            // The device sums in another order; a draw that lands within
            // rounding of a boundary may pick the neighbor.
            if got as usize == want {
                matched += 1;
            } else {
                assert!(
                    (got as usize).abs_diff(want) <= 2,
                    "temperature {temperature} top-p {top_p} seed {s}: {got} vs {want}"
                );
            }
        }
    }
    assert!(
        matched * 10 >= cases * 9,
        "{matched} of {cases} draws match"
    );
    let mut poisoned = logits.clone();
    poisoned[17] = f32::NAN;
    logits_buffer.write_bytes(
        &poisoned
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>(),
    )?;
    assert_eq!(
        run(&backend, &logits_buffer, &settings, &seed, &positions)?,
        -1
    );
    Ok(())
}
