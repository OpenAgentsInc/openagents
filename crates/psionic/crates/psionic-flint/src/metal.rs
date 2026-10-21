//! Flint on Apple silicon: Recursive's MLX affine INT4 checkpoint
//! (`RecursiveSI/flint-int4`) loaded straight from its safetensors shards and
//! run by `psionic_backend_metal::flint`, whose kernels adapt the decode
//! design of Recursive's `rec` engine (Apache-2.0).
//!
//! The model definition is shared with the BF16 path: [`Config`], the chat
//! template, the tokenizer, [`Sampling`], stop tokens and [`split_answer`].
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, bail, ensure};
use psionic_backend_metal::flint::{
    FlintLayer, FlintMetal, FlintMlp, FlintQuant, FlintSession, FlintTensor, FlintWeights,
};
use rand::{SeedableRng, rngs::StdRng};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{Config, Generation, STOP_IDS, Sampling, split_answer};

pub const INT4_MODEL_ID: &str = "RecursiveSI/flint-int4";
pub const INT4_REVISION: &str = "3b93f9053b530488405ae9fca51d4900d6ce21f0";

#[derive(Deserialize)]
struct PinnedFile {
    path: String,
    size: u64,
    sha256: String,
}
#[derive(Deserialize)]
struct Artifact {
    files: Vec<PinnedFile>,
}

fn artifact() -> Result<Artifact> {
    Ok(serde_json::from_str(include_str!(
        "../testdata/mlx-int4-artifact.json"
    ))?)
}

/// The pinned shard and metadata files, with sizes and SHA-256 digests.
pub fn pinned_files() -> Result<Vec<(String, u64, String)>> {
    Ok(artifact()?
        .files
        .into_iter()
        .map(|f| (f.path, f.size, f.sha256))
        .collect())
}

/// Check sizes always and SHA-256 digests when `digests` is set (about 22.6 GB).
pub fn verify_int4_artifact(root: &Path, digests: bool) -> Result<()> {
    let files = artifact()?.files;
    let checks: Vec<_> = files
        .into_iter()
        .map(|file| {
            let path = root.join(&file.path);
            std::thread::spawn(move || -> Result<()> {
                let mut input =
                    fs::File::open(&path).with_context(|| format!("opening {}", file.path))?;
                ensure!(
                    input.metadata()?.len() == file.size,
                    "wrong size for {}",
                    file.path
                );
                if !digests {
                    return Ok(());
                }
                let mut hash = Sha256::new();
                let mut buffer = vec![0u8; 8 << 20];
                loop {
                    let n = input.read(&mut buffer)?;
                    if n == 0 {
                        break;
                    }
                    hash.update(&buffer[..n]);
                }
                ensure!(
                    format!("{:x}", hash.finalize()) == file.sha256,
                    "checksum mismatch for {}",
                    file.path
                );
                Ok(())
            })
        })
        .collect();
    for check in checks {
        check
            .join()
            .map_err(|_| anyhow::anyhow!("checksum thread failed"))??;
    }
    Ok(())
}

struct Entry {
    dtype: String,
    shape: Vec<usize>,
    tensor: FlintTensor,
}

/// Read every shard's safetensors header into a name table.
fn index(shards: &[PathBuf]) -> Result<BTreeMap<String, Entry>> {
    let mut table = BTreeMap::new();
    for (shard, path) in shards.iter().enumerate() {
        let mut file = fs::File::open(path)?;
        let mut size = [0u8; 8];
        file.read_exact(&mut size)?;
        let length = u64::from_le_bytes(size);
        ensure!(length < 64 << 20, "safetensors header too large");
        let mut header = vec![0u8; length as usize];
        file.read_exact(&mut header)?;
        let header: BTreeMap<String, serde_json::Value> = serde_json::from_slice(&header)?;
        for (name, value) in header {
            if name == "__metadata__" {
                continue;
            }
            let dtype = value["dtype"].as_str().context("tensor dtype")?.to_string();
            let shape: Vec<usize> = serde_json::from_value(value["shape"].clone())?;
            let offsets: Vec<u64> = serde_json::from_value(value["data_offsets"].clone())?;
            ensure!(offsets.len() == 2, "tensor offsets");
            let entry = Entry {
                dtype,
                shape,
                tensor: FlintTensor {
                    shard,
                    offset: 8 + length + offsets[0],
                },
            };
            ensure!(
                table.insert(name.clone(), entry).is_none(),
                "duplicate tensor {name}"
            );
        }
    }
    Ok(table)
}

struct Names<'a> {
    table: &'a BTreeMap<String, Entry>,
}

impl Names<'_> {
    fn tensor(&self, name: &str, dtype: &str, shape: &[usize]) -> Result<FlintTensor> {
        let entry = self
            .table
            .get(name)
            .with_context(|| format!("missing tensor {name}"))?;
        ensure!(
            entry.dtype == dtype && entry.shape == shape,
            "unexpected layout for {name}: {} {:?}",
            entry.dtype,
            entry.shape
        );
        Ok(entry.tensor)
    }
    /// An affine matrix of `rows` x `cols` at `bits`, optionally per expert.
    fn quant(
        &self,
        name: &str,
        experts: Option<usize>,
        rows: usize,
        cols: usize,
        bits: usize,
    ) -> Result<FlintQuant> {
        let lead: Vec<usize> = experts.into_iter().collect();
        let shape = |last: usize| [lead.clone(), vec![rows, last]].concat();
        Ok(FlintQuant {
            weight: self.tensor(&format!("{name}.weight"), "U8", &shape(cols * bits / 8))?,
            scales: self.tensor(&format!("{name}.scales"), "BF16", &shape(cols / 64))?,
            biases: self.tensor(&format!("{name}.biases"), "BF16", &shape(cols / 64))?,
        })
    }
}

/// Map the MLX checkpoint's tensors onto the Metal layout, checking every shape.
fn weights(c: &Config, table: &BTreeMap<String, Entry>) -> Result<FlintWeights> {
    let n = Names { table };
    let h = c.hidden_size;
    let mut layers = Vec::new();
    for id in 0..c.num_hidden_layers {
        let p = format!("model.layers.{id}");
        let a = format!("{p}.self_attn");
        let m = format!("{p}.mlp");
        let global = c.layer_types[id] == "full_attention";
        let rotary = !c.no_rope_layer_indices.contains(&id);
        ensure!(
            global != rotary,
            "layer {id}: Flint pairs full attention with NoPE"
        );
        let mlp = if c.mlp_only_layers.contains(&id) {
            FlintMlp::Dense {
                gate: n.quant(&format!("{m}.gate_proj"), None, c.intermediate_size, h, 4)?,
                up: n.quant(&format!("{m}.up_proj"), None, c.intermediate_size, h, 4)?,
                down: n.quant(&format!("{m}.down_proj"), None, h, c.intermediate_size, 4)?,
            }
        } else {
            let e = Some(c.num_experts);
            let f = c.moe_intermediate_size;
            let s = c.shared_expert_intermediate_size;
            FlintMlp::Sparse {
                router: n.tensor(&format!("{m}.gate.weight"), "BF16", &[c.num_experts, h])?,
                gate: n.quant(&format!("{m}.switch_mlp.gate_proj"), e, f, h, 4)?,
                up: n.quant(&format!("{m}.switch_mlp.up_proj"), e, f, h, 4)?,
                down: n.quant(&format!("{m}.switch_mlp.down_proj"), e, h, f, 4)?,
                shared_gate: n.quant(&format!("{m}.shared_expert.gate_proj"), None, s, h, 4)?,
                shared_up: n.quant(&format!("{m}.shared_expert.up_proj"), None, s, h, 4)?,
                shared_down: n.quant(&format!("{m}.shared_expert.down_proj"), None, h, s, 4)?,
            }
        };
        let qd = c.num_attention_heads * c.head_dim;
        let kd = c.num_key_value_heads * c.head_dim;
        layers.push(FlintLayer {
            global,
            rotary,
            input_norm: n.tensor(&format!("{p}.input_layernorm.weight"), "BF16", &[h])?,
            post_norm: n.tensor(
                &format!("{p}.post_attention_layernorm.weight"),
                "BF16",
                &[h],
            )?,
            q: n.quant(&format!("{a}.q_proj"), None, qd, h, 4)?,
            k: n.quant(&format!("{a}.k_proj"), None, kd, h, 4)?,
            v: n.quant(&format!("{a}.v_proj"), None, kd, h, 4)?,
            o: n.quant(&format!("{a}.o_proj"), None, h, qd, 4)?,
            q_norm: n.tensor(&format!("{a}.q_norm.weight"), "BF16", &[c.head_dim])?,
            k_norm: n.tensor(&format!("{a}.k_norm.weight"), "BF16", &[c.head_dim])?,
            sinks: n.tensor(
                &format!("{a}.softmax_offset"),
                "BF16",
                &[c.num_attention_heads],
            )?,
            mlp,
        });
    }
    Ok(FlintWeights {
        embed: n.quant("model.embed_tokens", None, c.vocab_size, h, 8)?,
        layers,
        norm: n.tensor("model.norm.weight", "BF16", &[h])?,
        head: n.quant("lm_head", None, c.vocab_size, h, 4)?,
        vocab: c.vocab_size,
    })
}

/// The dimensions the Metal kernels are written for.
fn check_dimensions(c: &Config) -> Result<()> {
    let dims = (
        c.hidden_size,
        c.num_attention_heads,
        c.num_key_value_heads,
        c.head_dim,
        c.num_experts,
        c.num_experts_per_tok,
        c.moe_intermediate_size,
        c.shared_expert_intermediate_size,
        c.intermediate_size,
        c.sliding_window,
    );
    ensure!(
        dims == (2048, 32, 4, 128, 256, 6, 512, 1024, 6144, 513),
        "the Metal Flint kernels need Flint's released dimensions"
    );
    ensure!(
        (c.rms_norm_eps - 1e-6).abs() < 1e-12 && c.rope_theta == 10000.0,
        "the Metal Flint kernels need epsilon 1e-6 and RoPE theta 10,000"
    );
    Ok(())
}

#[derive(Deserialize)]
struct Quantization {
    group_size: usize,
    bits: usize,
}
#[derive(Deserialize)]
struct Export {
    bits: usize,
    embedding_bits: usize,
    routed_expert_bits: usize,
}
#[derive(Deserialize)]
struct MlxConfig {
    quantization: Quantization,
    mlx_export: Export,
}

/// Flint with MLX INT4 weights on the GPU.
pub struct MetalModel {
    pub config: Config,
    pub config_sha256: String,
    engine: FlintMetal,
    pub load_seconds: f64,
}

impl MetalModel {
    /// Load `RecursiveSI/flint-int4` from `root`. Digests are checked when `verify` is set.
    pub fn load(root: &Path, verify: bool) -> Result<Self> {
        let started = Instant::now();
        verify_int4_artifact(root, verify)?;
        let bytes = fs::read(root.join("config.json"))?;
        let config: Config = serde_json::from_slice(&bytes)?;
        config.validate()?;
        check_dimensions(&config)?;
        let mlx: MlxConfig = serde_json::from_slice(&bytes)?;
        ensure!(
            (mlx.quantization.group_size, mlx.quantization.bits) == (64, 4)
                && (
                    mlx.mlx_export.bits,
                    mlx.mlx_export.embedding_bits,
                    mlx.mlx_export.routed_expert_bits
                ) == (4, 8, 4),
            "expected group-64 INT4 projections and experts with INT8 embeddings"
        );
        let shards: Vec<PathBuf> = artifact()?
            .files
            .iter()
            .filter(|f| f.path.ends_with(".safetensors"))
            .map(|f| root.join(&f.path))
            .collect();
        let table = index(&shards)?;
        let weights = weights(&config, &table)?;
        let engine = FlintMetal::load(&shards, weights).map_err(|e| anyhow::anyhow!(e))?;
        Ok(Self {
            config,
            config_sha256: format!("{:x}", Sha256::digest(&bytes)),
            engine,
            load_seconds: started.elapsed().as_secs_f64(),
        })
    }

    pub fn device_name(&self) -> &str {
        self.engine.device_name()
    }

    pub fn weight_bytes(&self) -> u64 {
        self.engine.weight_bytes()
    }

    pub fn session(&self, capacity: usize) -> Result<FlintSession> {
        ensure!(
            capacity > 0 && capacity <= self.config.max_position_embeddings,
            "context exceeds model capacity"
        );
        self.engine
            .session(capacity)
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Prefill a prompt; returns the greedy next token. Logits via [`Self::logits`].
    pub fn prefill(&self, session: &mut FlintSession, tokens: &[u32]) -> Result<u32> {
        self.engine
            .prefill(session, tokens)
            .map_err(|e| anyhow::anyhow!(e))
    }

    /// Run one token; returns the greedy next token.
    pub fn decode(&self, session: &mut FlintSession, token: u32) -> Result<u32> {
        self.engine
            .decode(session, token)
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn logits(&self, out: &mut Vec<f32>) {
        self.engine.logits(out)
    }
}

/// Per-request timing beyond [`Generation`].
#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct Timing {
    pub first_token_seconds: f64,
    pub decode_seconds: f64,
    pub decode_tokens_per_second: f64,
    pub prefill_tokens_per_second: f64,
}

/// Generate with the Metal path. Greedy when `temperature` is zero.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    model: &MetalModel,
    tokenizer: &tokenizers::Tokenizer,
    prompt: &[u32],
    max_tokens: usize,
    capacity: usize,
    sampling: &Sampling,
    deadline_seconds: f64,
    mut on_token: impl FnMut(u32),
) -> Result<(Generation, Timing)> {
    sampling.validate()?;
    ensure!(
        !prompt.is_empty()
            && max_tokens > 0
            && prompt
                .len()
                .checked_add(max_tokens)
                .is_some_and(|n| n <= capacity),
        "prompt and output exceed context limit"
    );
    ensure!(
        deadline_seconds.is_finite() && deadline_seconds > 0.0,
        "invalid request deadline"
    );
    let start = Instant::now();
    let mut session = model.session(capacity)?;
    let greedy = sampling.temperature == 0.0;
    let mut rng = StdRng::seed_from_u64(sampling.seed);
    let mut logits = Vec::new();
    let mut choose = |gpu_choice: u32, logits: &mut Vec<f32>| -> Result<u32> {
        if greedy {
            return Ok(gpu_choice);
        }
        model.logits(logits);
        sampling.choose(logits, &mut rng)
    };
    let mut token = choose(model.prefill(&mut session, prompt)?, &mut logits)?;
    let prefill_seconds = start.elapsed().as_secs_f64();
    let mut first = None;
    let mut ids = Vec::new();
    let mut finish = "length";
    for step in 0..max_tokens {
        if first.is_none() {
            first = Some(start.elapsed().as_secs_f64());
        }
        if STOP_IDS.contains(&token) {
            finish = "stop";
            break;
        }
        ids.push(token);
        on_token(token);
        if step + 1 == max_tokens {
            break;
        }
        if start.elapsed().as_secs_f64() >= deadline_seconds {
            bail!("request timed out");
        }
        token = choose(model.decode(&mut session, token)?, &mut logits)?;
    }
    let elapsed = start.elapsed().as_secs_f64();
    let first = first.unwrap_or(elapsed);
    let decode_seconds = elapsed - first;
    let text = tokenizer
        .decode(&ids, false)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let timing = Timing {
        first_token_seconds: first,
        decode_seconds,
        decode_tokens_per_second: if decode_seconds > 0.0 && ids.len() > 1 {
            (ids.len() - 1) as f64 / decode_seconds
        } else {
            0.0
        },
        prefill_tokens_per_second: prompt.len() as f64 / prefill_seconds.max(1e-9),
    };
    Ok((
        Generation {
            answer: split_answer(&text)?,
            token_ids: ids,
            prompt_tokens: prompt.len(),
            finish_reason: finish.to_string(),
            elapsed_seconds: elapsed,
            prefill_seconds,
        },
        timing,
    ))
}
