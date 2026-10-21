//! Runs a real DeepSeek-V4.1 stage on one GPU (#12394, W7) and checks it.
//!
//! ```text
//! deepseek_v41_stage_run --checkpoint DIR --oracle DIR --out DIR
//!     [--layers 0-5] [--prompts p1-chat-thinking,p3-2k] [--cpu p1-chat-thinking]
//!     [--chunk 64] [--max-context 4096]
//! ```
//!
//! The checkpoint directory holds the pinned index and every shard the stage
//! reads; other shards may be sparse stand-ins with their real headers. For
//! each prompt the stage prefills in chunks and then decodes the oracle's
//! generated tokens, teacher-forced, once with plain graphs for timing and
//! once with observations. Its observations are compared with the oracle's
//! decode-mode tensors under the frozen W1 policy. For the `--cpu` prompt,
//! the CPU path in the same `ReferenceBf16` order runs the same layers, and
//! the GPU is compared with it layer by layer and with the oracle. The report
//! records per-token decode latency, prefill throughput, device memory, the
//! host round trips of each step, and the boundary's wire bytes.

#[path = "../src/v41_stage_reference.rs"]
mod reference;

use psionic_backend_cuda::CudaBackend;
use psionic_backend_cuda::v41_stage::{
    V41EngramHash, V41EngramHostTable, V41HostTensor, V41Sampling, V41Site, V41Stage,
    V41StageConfig, V41StageShape, V41StepInput, V41StepOutput, V41TensorDtype, V41TensorSource,
    V41TokenMap,
};
use psionic_models::deepseek_v41_cpu::{
    block::BlockConfig,
    engram::{CompressedTokenMap, EngramLayout},
};
use psionic_models::deepseek_v41_stage::{V41StageGeometry, stage_contract, zero_decode_message};
use psionic_models::deepseek_v41_weights::{
    CheckpointOpenOptions, CheckpointPin, DeepSeekV41Checkpoint, SafetensorsDtype, decode_bf16,
    sha256_file,
};
use psionic_runtime::RuntimeError;
use reference::{CpuStage, Observations};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const POLICY: &str = include_str!(
    "../../psionic-models/src/testdata/deepseek_v41/cpu_qualification/tolerance-policy.json"
);
const TOKENIZER_SHA256: &str = "c90dfa01249db1be4245780a052ede752e1361c612ac6d08e2bdada7d599476b";

struct Checkpoint(DeepSeekV41Checkpoint);

impl V41TensorSource for Checkpoint {
    fn tensor(&self, name: &str) -> std::result::Result<V41HostTensor, RuntimeError> {
        let e = |error: &dyn std::fmt::Display| RuntimeError::Backend(format!("{name}: {error}"));
        let entry = self.0.entry(name).map_err(|x| e(&x))?;
        let dtype = match entry.dtype {
            SafetensorsDtype::F32 => V41TensorDtype::F32,
            SafetensorsDtype::Bf16 => V41TensorDtype::Bf16,
            SafetensorsDtype::F8E4M3 => V41TensorDtype::F8E4M3,
            SafetensorsDtype::F8E8M0 => V41TensorDtype::F8E8M0,
            SafetensorsDtype::I8 => V41TensorDtype::I8,
        };
        Ok(V41HostTensor {
            dtype,
            shape: entry.shape.iter().map(|d| *d as usize).collect(),
            bytes: self.0.read_tensor(name).map_err(|x| e(&x))?,
        })
    }
}

struct Args {
    checkpoint: PathBuf,
    oracle: PathBuf,
    out: PathBuf,
    layers: std::ops::Range<usize>,
    prompts: Vec<String>,
    cpu: Option<String>,
    chunk: usize,
    max_context: usize,
    skip_hashes: bool,
    timing_only: bool,
}

fn args() -> Result<Args> {
    let mut a = std::env::args().skip(1);
    let mut out = Args {
        checkpoint: PathBuf::new(),
        oracle: PathBuf::new(),
        out: PathBuf::new(),
        layers: 0..5,
        prompts: vec![String::from("p1-chat-thinking")],
        cpu: None,
        chunk: 64,
        max_context: 4096,
        skip_hashes: false,
        timing_only: false,
    };
    while let Some(flag) = a.next() {
        let mut value = || a.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--checkpoint" => out.checkpoint = value()?.into(),
            "--oracle" => out.oracle = value()?.into(),
            "--out" => out.out = value()?.into(),
            "--layers" => {
                let v = value()?;
                let (a, b) = v.split_once('-').ok_or("--layers wants START-END")?;
                out.layers = a.parse()?..b.parse()?;
            }
            "--prompts" => out.prompts = value()?.split(',').map(String::from).collect(),
            "--cpu" => out.cpu = Some(value()?),
            "--chunk" => out.chunk = value()?.parse()?,
            "--max-context" => out.max_context = value()?.parse()?,
            "--skip-hashes" => out.skip_hashes = true,
            "--timing-only" => out.timing_only = true,
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    if out.checkpoint.as_os_str().is_empty()
        || out.oracle.as_os_str().is_empty()
        || out.out.as_os_str().is_empty()
    {
        return Err("--checkpoint, --oracle, and --out are required".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// The oracle dump (read as the CPU qualification example reads it)
// ---------------------------------------------------------------------------

struct OracleEntry {
    file: String,
    shape: Vec<usize>,
    dtype: String,
    bytes: usize,
    sha256: String,
}

struct Oracle {
    root: PathBuf,
    entries: BTreeMap<String, OracleEntry>,
}

impl Oracle {
    fn open(root: &Path) -> Result<Self> {
        let m: Value = serde_json::from_slice(&fs::read(root.join("manifest.json"))?)?;
        let mut entries = BTreeMap::new();
        for t in m["tensors"].as_array().ok_or("oracle tensors")? {
            entries.insert(
                t["name"].as_str().ok_or("name")?.to_owned(),
                OracleEntry {
                    file: t["file"].as_str().ok_or("file")?.to_owned(),
                    shape: serde_json::from_value(t["shape"].clone())?,
                    dtype: t["dtype"].as_str().ok_or("dtype")?.to_owned(),
                    bytes: t["bytes"].as_u64().ok_or("bytes")? as usize,
                    sha256: t["sha256"].as_str().ok_or("sha")?.to_owned(),
                },
            );
        }
        Ok(Self {
            root: root.to_owned(),
            entries,
        })
    }

    fn values(&self, name: &str) -> Result<Vec<f64>> {
        let e = self
            .entries
            .get(name)
            .ok_or_else(|| format!("no oracle tensor {name}"))?;
        let mut file = File::open(self.root.join(&e.file))?;
        let mut size = [0; 8];
        file.read_exact(&mut size)?;
        let size = u64::from_le_bytes(size);
        let mut header = vec![0; usize::try_from(size)?];
        file.read_exact(&mut header)?;
        let h: Value = serde_json::from_slice(&header)?;
        let begin = h[name]["data_offsets"][0].as_u64().ok_or("offsets")?;
        file.seek(SeekFrom::Start(8 + size + begin))?;
        let mut raw = vec![0; e.bytes];
        file.read_exact(&mut raw)?;
        if hex::encode(Sha256::digest(&raw)) != e.sha256 {
            return Err(format!("oracle SHA-256 mismatch: {name}").into());
        }
        let _ = &e.shape;
        Ok(match e.dtype.as_str() {
            "bfloat16" => raw
                .chunks_exact(2)
                .map(|x| f64::from(decode_bf16([x[0], x[1]])))
                .collect(),
            "float32" => raw
                .chunks_exact(4)
                .map(|x| f64::from(f32::from_le_bytes([x[0], x[1], x[2], x[3]])))
                .collect(),
            "int64" => raw
                .chunks_exact(8)
                .map(|x| i64::from_le_bytes(x.try_into().unwrap_or([0; 8])) as f64)
                .collect(),
            "int32" => raw
                .chunks_exact(4)
                .map(|x| f64::from(i32::from_le_bytes([x[0], x[1], x[2], x[3]])))
                .collect(),
            other => return Err(format!("oracle dtype {other}").into()),
        })
    }
}

// ---------------------------------------------------------------------------
// Comparison under the frozen policy
// ---------------------------------------------------------------------------

fn family(site: &str) -> &'static str {
    if site.ends_with(".topk_idxs") || site.ends_with(".ids") || site.ends_with(".hash_ids") {
        "integer_and_hash"
    } else if site == "embed.out" {
        "embedding"
    } else if [
        ".pre",
        ".post",
        ".comb",
        ".pre_mix_in",
        ".pre_mix_out",
        ".gate.weights",
    ]
    .iter()
    .any(|s| site.ends_with(s))
    {
        "float32_mixing_and_router"
    } else {
        "bf16_activation_and_residual"
    }
}

fn limits(policy: &Value, family: &str) -> (f64, f64) {
    let p = &policy["families"][family];
    if p["exact"] == true {
        (0.0, 0.0)
    } else {
        (
            p["atol"].as_f64().unwrap_or(0.0),
            p["rtol"].as_f64().unwrap_or(0.0),
        )
    }
}

/// `actual` against `reference` under `abs <= atol + rtol |reference|`.
fn differences(actual: &[f64], reference: &[f64], atol: f64, rtol: f64) -> Value {
    let mut bad = actual.len().abs_diff(reference.len());
    let (mut unequal, mut worst, mut max_abs) = (0, 0.0_f64, 0.0_f64);
    let mut first = Value::Null;
    for (i, (a, b)) in actual.iter().zip(reference).enumerate() {
        if a.to_bits() != b.to_bits() {
            unequal += 1;
        }
        let error = (a - b).abs();
        let limit = atol + rtol * b.abs();
        max_abs = max_abs.max(error);
        if limit > 0.0 {
            worst = worst.max(error / limit);
        }
        if !a.is_finite() || !b.is_finite() || error > limit {
            bad += 1;
            if first.is_null() {
                first = json!({"index": i, "actual": a, "reference": b, "abs_error": error});
            }
        }
    }
    json!({"passed": bad == 0, "elements": reference.len(), "bad": bad, "unequal": unequal,
        "max_abs_error": max_abs, "worst_over_limit": worst, "first_bad": first})
}

/// The values of one site for one token of a step, as `f64`.
fn gpu_site(
    observed: &BTreeMap<usize, BTreeMap<V41Site, Vec<u32>>>,
    layer: usize,
    site: V41Site,
    token: usize,
    tokens: usize,
) -> Option<Vec<f64>> {
    let words = observed.get(&layer)?.get(&site)?;
    let per = words.len() / tokens;
    let row = &words[token * per..(token + 1) * per];
    Some(match site {
        V41Site::EngramIds => row
            .chunks_exact(2)
            .map(|w| (u64::from(w[0]) | (u64::from(w[1]) << 32)) as i64 as f64)
            .collect(),
        V41Site::ExpertIds | V41Site::Selection => {
            row.iter().map(|w| f64::from(*w as i32)).collect()
        }
        _ => row.iter().map(|w| f64::from(f32::from_bits(*w))).collect(),
    })
}

fn cpu_site(
    observed: &Observations,
    layer: usize,
    site: V41Site,
    token: usize,
    tokens: usize,
) -> Option<Vec<f64>> {
    let values = observed.get(&layer)?.get(&site)?;
    let per = values.len() / tokens;
    Some(values[token * per..(token + 1) * per].to_vec())
}

/// Oracle names for a stage observation: `(site name, slice of the values)`.
fn oracle_sites(site: V41Site, streams: usize) -> Vec<(&'static str, std::ops::Range<usize>)> {
    let m = |a: usize, b: usize| a..b;
    let s = streams;
    match site {
        V41Site::EngramIds => vec![("engram.hash_ids", m(0, usize::MAX))],
        V41Site::EngramRows => vec![("engram.rows", m(0, usize::MAX))],
        V41Site::EngramOut => vec![("engram.out", m(0, usize::MAX))],
        V41Site::HcIn => vec![("hc.in", m(0, usize::MAX))],
        V41Site::AttnMixes => vec![
            ("hc.attn.pre", m(0, s)),
            ("hc.attn.post", m(s, 2 * s)),
            ("hc.attn.comb", m(2 * s, usize::MAX)),
        ],
        V41Site::AttnIn => vec![("attn.in", m(0, usize::MAX))],
        V41Site::AttnOut => vec![("attn.out", m(0, usize::MAX))],
        V41Site::AttnExpanded => vec![("hc.attn.expanded", m(0, usize::MAX))],
        V41Site::FfnMixes => vec![
            ("hc.ffn.pre", m(0, s)),
            ("hc.pre_mix_out", m(0, s)),
            ("hc.ffn.post", m(s, 2 * s)),
            ("hc.ffn.comb", m(2 * s, usize::MAX)),
        ],
        V41Site::FfnIn => vec![("ffn.gate.in", m(0, usize::MAX))],
        V41Site::FfnOut => vec![("ffn.out", m(0, usize::MAX))],
        V41Site::HcOut => vec![
            ("hc.out", m(0, usize::MAX)),
            ("hc.ffn.expanded", m(0, usize::MAX)),
        ],
        // Ids, weights, and selections need their own alignment.
        V41Site::ExpertIds | V41Site::RouteWeights | V41Site::Selection => Vec::new(),
    }
}

const SITES: [V41Site; 15] = [
    V41Site::EngramIds,
    V41Site::EngramRows,
    V41Site::EngramOut,
    V41Site::HcIn,
    V41Site::AttnMixes,
    V41Site::AttnIn,
    V41Site::AttnOut,
    V41Site::AttnExpanded,
    V41Site::FfnMixes,
    V41Site::FfnIn,
    V41Site::ExpertIds,
    V41Site::RouteWeights,
    V41Site::FfnOut,
    V41Site::HcOut,
    V41Site::Selection,
];

struct Case {
    id: String,
    prompt: Vec<u32>,
    generated: Vec<u32>,
    labels: BTreeMap<usize, String>,
    run: Value,
}

impl Case {
    fn load(oracle: &Path, id: &str) -> Result<Self> {
        let run: Value = serde_json::from_slice(&fs::read(oracle.join(id).join("run.json"))?)?;
        let prompt: Vec<u32> = serde_json::from_value(run["prompt_token_ids"].clone())?;
        let generated: Vec<u32> = serde_json::from_value(run["generated_ids"].clone())?;
        let mut labels: BTreeMap<usize, String> = run["labels"]
            .as_object()
            .ok_or("labels")?
            .iter()
            .filter_map(|(k, v)| v.as_u64().map(|p| (p as usize, k.clone())))
            .collect();
        for k in 1..=generated.len() {
            labels.insert(prompt.len() + k - 1, format!("g{k}"));
        }
        Ok(Self {
            id: id.to_owned(),
            prompt,
            generated,
            labels,
            run,
        })
    }

    /// The steps: the prompt in chunks, then one generated token each.
    fn steps(&self, chunk: usize) -> Vec<(usize, Vec<u32>)> {
        let mut out = Vec::new();
        let mut position = 0;
        for c in self.prompt.chunks(chunk) {
            out.push((position, c.to_vec()));
            position += c.len();
        }
        for t in &self.generated {
            out.push((position, vec![*t]));
            position += 1;
        }
        out
    }

    fn exact_tie(&self, layer: usize, row: usize, step: usize, kind: &str) -> bool {
        self.run["near_ties"].as_array().is_some_and(|ties| {
            ties.iter().any(|t| {
                t["mode"] == "decode"
                    && t["step"].as_u64() == Some(step as u64)
                    && t["layer"].as_u64() == Some(layer as u64)
                    && t["row"].as_u64() == Some(row as u64)
                    && t["kind"] == kind
                    && t["margin"].as_f64() == Some(0.0)
            })
        })
    }
}

struct Tally {
    checked: usize,
    failed: usize,
    first_failure: Option<Value>,
    rows: Vec<Value>,
}

impl Tally {
    fn new() -> Self {
        Self {
            checked: 0,
            failed: 0,
            first_failure: None,
            rows: Vec::new(),
        }
    }
    fn add(&mut self, mut row: Value, name: &str) {
        row["tensor"] = json!(name);
        self.checked += 1;
        if row["passed"] != true {
            self.failed += 1;
            if self.first_failure.is_none() {
                self.first_failure = Some(row.clone());
            }
        }
        self.rows.push(row);
    }
    fn summary(&self) -> Value {
        json!({"checked": self.checked, "failed": self.failed, "first_failure": self.first_failure})
    }
}

/// Compares one step's observations of one side with the oracle.
#[allow(clippy::too_many_arguments)]
fn compare_with_oracle(
    oracle: &Oracle,
    case: &Case,
    policy: &Value,
    position: usize,
    tokens: usize,
    oracle_step: usize,
    layers: &std::ops::Range<usize>,
    streams: usize,
    window: usize,
    get: &dyn Fn(usize, V41Site, usize) -> Option<Vec<f64>>,
    tally: &mut Tally,
) -> Result<()> {
    let prefix = format!("{}.decode", case.id);
    for t in 0..tokens {
        let p = position + t;
        // Router sets at every position.
        for layer in layers.clone() {
            let all = format!("{prefix}.router_ids.layers.{layer}");
            if let (Some(ids), true) = (
                get(layer, V41Site::ExpertIds, t),
                oracle.entries.contains_key(&all),
            ) {
                let reference = oracle.values(&all)?;
                let k = ids.len();
                if let Some(row) = reference.get(p * k..(p + 1) * k) {
                    let mut expected: Vec<i64> = row.iter().map(|x| *x as i64).collect();
                    expected.sort_unstable();
                    let actual: Vec<i64> = ids.iter().map(|x| *x as i64).collect();
                    let tie = case.exact_tie(layer, p - position.min(p), oracle_step, "gate");
                    let passed = actual == expected || tie;
                    tally.add(
                        json!({"kind": "router_set", "layer": layer, "position": p, "passed": passed,
                            "exact_tie_waiver": tie && actual != expected, "actual": actual, "reference": expected}),
                        &all,
                    );
                }
            }
        }
        let Some(label) = case.labels.get(&p) else {
            continue;
        };
        let base = format!("{prefix}.{label}");
        if layers.start == 0 {
            let name = format!("{base}.embed.out");
            if let (Some(hc), true) = (get(0, V41Site::HcIn, t), oracle.entries.contains_key(&name))
            {
                let reference = oracle.values(&name)?;
                let (atol, rtol) = limits(policy, "embedding");
                // The first stream of layer 0's input is the embedding (no
                // Engram at layer 0).
                tally.add(
                    differences(&hc[..reference.len()], &reference, atol, rtol),
                    &name,
                );
            }
        }
        for layer in layers.clone() {
            for site in SITES {
                let Some(values) = get(layer, site, t) else {
                    continue;
                };
                for (name, range) in oracle_sites(site, streams) {
                    let full = format!("{base}.layers.{layer}.{name}");
                    if !oracle.entries.contains_key(&full) {
                        continue;
                    }
                    let reference = oracle.values(&full)?;
                    let end = range.end.min(values.len());
                    let actual = &values[range.start..end];
                    let (atol, rtol) = limits(policy, family(&full));
                    tally.add(differences(actual, &reference, atol, rtol), &full);
                }
            }
            // Route weights in the oracle's id order.
            let ids_name = format!("{base}.layers.{layer}.ffn.gate.ids");
            let weights_name = format!("{base}.layers.{layer}.ffn.gate.weights");
            if let (Some(ids), Some(weights), true, true) = (
                get(layer, V41Site::ExpertIds, t),
                get(layer, V41Site::RouteWeights, t),
                oracle.entries.contains_key(&ids_name),
                oracle.entries.contains_key(&weights_name),
            ) {
                let order = oracle.values(&ids_name)?;
                let reference = oracle.values(&weights_name)?;
                let aligned: Option<Vec<f64>> = order
                    .iter()
                    .map(|id| ids.iter().position(|x| x == id).map(|i| weights[i]))
                    .collect();
                let (atol, rtol) = limits(policy, "float32_mixing_and_router");
                let row = match aligned {
                    Some(a) => differences(&a, &reference, atol, rtol),
                    None => json!({"passed": false, "reason": "router set differs"}),
                };
                tally.add(row, &weights_name);
            }
            // Index selections, with the reference's KV offset.
            let sel_name = format!("{base}.layers.{layer}.attn.indexer.topk_idxs");
            if let (Some(sel), true) = (
                get(layer, V41Site::Selection, t),
                oracle.entries.contains_key(&sel_name),
            ) {
                let reference = oracle.values(&sel_name)?;
                // The oracle prefilled the whole prompt in one forward, whose KV
                // array starts after the prompt; decode steps start after the
                // window.
                let offset = if p < case.prompt.len() {
                    case.prompt.len()
                } else {
                    window
                } as f64;
                let actual: Vec<f64> = sel
                    .iter()
                    .take(reference.len())
                    .map(|i| if *i < 0.0 { -1.0 } else { i + offset })
                    .collect();
                let tie = case.exact_tie(layer, t, oracle_step, "indexer");
                let mut row = differences(&actual, &reference, 0.0, 0.0);
                if row["passed"] != true && tie {
                    row["passed"] = json!(true);
                    row["exact_tie_waiver"] = json!(true);
                }
                tally.add(row, &sel_name);
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------

fn write(out: &Path, report: &Value) -> Result<()> {
    fs::write(out.join("report.json"), serde_json::to_vec_pretty(report)?)?;
    Ok(())
}

fn engram_table(
    checkpoint: &DeepSeekV41Checkpoint,
    backend: &mut CudaBackend,
    layer: usize,
) -> Result<V41EngramHostTable> {
    let fill =
        |backend: &mut CudaBackend, name: &str| -> Result<psionic_backend_cuda::CudaHostBuffer> {
            let entry = checkpoint.entry(name)?;
            let shard = checkpoint.shard_of(name).ok_or("Engram shard")?;
            let file = File::open(checkpoint.root().join(shard))?;
            let len = usize::try_from(entry.byte_len)?;
            let mut host = backend.host_buffer(len)?;
            let mut buffer = vec![0_u8; 256 << 20];
            let mut done = 0;
            while done < len {
                let n = buffer.len().min(len - done);
                file.read_exact_at(&mut buffer[..n], entry.file_offset + done as u64)?;
                host.write_bytes_at_offset(done, &buffer[..n])?;
                done += n;
            }
            Ok(host)
        };
    let codes = fill(backend, &format!("layers.{layer}.engram.embed.weight"))?;
    let scales = fill(backend, &format!("layers.{layer}.engram.embed.scale"))?;
    let rows = checkpoint
        .entry(&format!("layers.{layer}.engram.embed.weight"))?
        .shape[0];
    Ok(V41EngramHostTable {
        layer,
        codes,
        scales,
        rows,
    })
}

fn memory(backend: &CudaBackend) -> Value {
    match backend.device_memory_info() {
        Ok(Some(m)) => {
            json!({"free": m.free_bytes, "total": m.total_bytes, "used": m.total_bytes - m.free_bytes})
        }
        _ => Value::Null,
    }
}

fn stats_json(out: &V41StepOutput) -> Value {
    let s = out.stats;
    json!({"tokens": s.tokens, "seconds": s.seconds, "device_seconds": s.device_seconds,
        "graph_launches": s.graph_launches, "graph_captures": s.graph_captures,
        "host_to_device_copies": s.host_to_device_copies, "host_to_device_bytes": s.host_to_device_bytes,
        "device_to_host_copies": s.device_to_host_copies, "device_to_host_bytes": s.device_to_host_bytes,
        "synchronizations": s.synchronizations})
}

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    let a = args()?;
    fs::create_dir_all(&a.out)?;
    let policy: Value = serde_json::from_str(POLICY)?;
    let started = Instant::now();
    let mut report = json!({
        "schema": "psionic.deepseek_v41.stage_run.v1",
        "issue": 12394,
        "layers": [a.layers.start, a.layers.end],
        "arithmetic": "reference_bf16 (default order)",
        "policy_sha256": hex::encode(Sha256::digest(POLICY.as_bytes())),
        "source_commit": std::env::var("V41_SOURCE_COMMIT").ok(),
        "prompts": a.prompts,
        "cpu_prompt": a.cpu,
        "chunk": a.chunk,
    });
    let checkpoint =
        DeepSeekV41Checkpoint::open(&a.checkpoint, CheckpointOpenOptions::pinned_flash()?)?;
    // Hash the shards this stage reads; the rest are sparse stand-ins.
    let mut needed = BTreeSet::new();
    for (name, shard) in &checkpoint.index().weight_map {
        let parts: Vec<&str> = name.split('.').collect();
        let in_stage = parts[0] == "layers"
            && parts
                .get(1)
                .and_then(|l| l.parse::<usize>().ok())
                .is_some_and(|l| a.layers.contains(&l));
        let global = (name == "embed.weight" && a.layers.start == 0)
            || ((name == "head.weight" || name == "norm.weight") && a.layers.end == 40);
        if in_stage || global {
            needed.insert(shard.clone());
        }
    }
    if !a.skip_hashes {
        let pin = CheckpointPin::deepseek_v41_flash()?;
        let t = Instant::now();
        let results: Vec<(String, std::result::Result<String, String>)> =
            std::thread::scope(|scope| {
                let handles: Vec<_> = needed
                    .iter()
                    .map(|shard| {
                        let path = a.checkpoint.join(shard);
                        let shard = shard.clone();
                        scope.spawn(move || (shard, sha256_file(&path).map_err(|e| e.to_string())))
                    })
                    .collect();
                handles.into_iter().filter_map(|h| h.join().ok()).collect()
            });
        for (shard, hash) in &results {
            let want = &pin.shards.get(shard).ok_or("pinned shard")?.sha256;
            if hash.as_ref().ok() != Some(want) {
                return Err(format!("{shard} does not match its pinned SHA-256: {hash:?}").into());
            }
        }
        report["shard_hashes"] = json!({"verified": needed, "seconds": t.elapsed().as_secs_f64()});
    }
    let tokenizer_bytes = fs::read(a.checkpoint.join("tokenizer.json"))?;
    if hex::encode(Sha256::digest(&tokenizer_bytes)) != TOKENIZER_SHA256 {
        return Err("pinned tokenizer SHA-256 mismatch".into());
    }
    let tokenizer =
        tokenizers::Tokenizer::from_bytes(&tokenizer_bytes).map_err(|e| e.to_string())?;
    let map = Arc::new(CompressedTokenMap::build(&tokenizer, 99_092)?);
    let pad = map.get(2)?;
    let layout = Arc::new(EngramLayout::flash()?);
    let oracle = Oracle::open(&a.oracle)?;
    let shape = V41StageShape::flash();
    let streams = shape.hc.streams;

    // The boundary's wire bytes, from W9's codec on real shapes.
    if a.layers.end < 40 {
        let contract = stage_contract(&V41StageGeometry::flash(), 1, a.layers.end as u32..40);
        let mut sizes = Vec::new();
        for position in [40_u64, 41] {
            let m = zero_decode_message(&contract, position, 36)?;
            sizes.push(json!({"position": position, "bytes": m.wire_bytes(),
                "sections": m.sections.iter().map(|s| json!([s.kind.to_string(), s.wire_bytes()])).collect::<Vec<_>>()}));
        }
        report["boundary_wire_bytes"] = json!(sizes);
    }
    report["token_return_bytes"] = json!(4);

    // ---------------- GPU ----------------
    let mut backend = CudaBackend::new();
    if backend.selected_device().is_none() || !backend.quantized_kernels_available() {
        return Err("no CUDA device or V4.1 kernels".into());
    }
    let memory_before = memory(&backend);
    let t = Instant::now();
    let mut tables = Vec::new();
    for layer in [1_usize, 14].into_iter().filter(|l| a.layers.contains(l)) {
        tables.push(engram_table(&checkpoint, &mut backend, layer)?);
    }
    let engram_seconds = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let source = Checkpoint(checkpoint);
    let mut stage = V41Stage::load(
        &mut backend,
        V41StageConfig {
            shape,
            layers: a.layers.clone(),
            max_context: a.max_context,
            max_step_tokens: a.chunk,
            engram: layout
                .layers
                .iter()
                .map(|l| V41EngramHash {
                    layer: l.layer_id as usize,
                    multipliers: l.multipliers,
                    primes: l.primes.clone(),
                    offsets: l.offsets.clone(),
                    rows: l.table_rows,
                })
                .collect(),
            token_map: Some(V41TokenMap {
                ids: map.ids().to_vec(),
                pad,
            }),
            observe: true,
            swap_rope_tables: false,
            experts_on_host: false,
        },
        &source,
        tables,
    )?;
    let (weight_bytes, state_bytes) = stage.device_bytes();
    report["load"] = json!({"engram_host_seconds": engram_seconds, "stage_seconds": t.elapsed().as_secs_f64(),
        "weight_bytes": weight_bytes, "state_bytes": state_bytes,
        "memory_before": memory_before, "memory_loaded": memory(&backend)});
    write(&a.out, &report)?;

    let mut gpu_observed: BTreeMap<
        String,
        Vec<(usize, usize, BTreeMap<usize, BTreeMap<V41Site, Vec<u32>>>)>,
    > = BTreeMap::new();
    let mut runs = Vec::new();
    for id in &a.prompts {
        let case = Case::load(&a.oracle, id)?;
        let steps = case.steps(a.chunk);
        // Timing with plain graphs.
        let mut timing = Vec::new();
        let mut boundary_bytes = Vec::new();
        for (position, tokens) in &steps {
            let out = stage.step(&backend, &input(*position, tokens), false)?;
            boundary_bytes.push(out.residual.len() * 2 + out.pre_mix.len() * 4);
            timing.push(stats_json(&out));
        }
        let prefill: Vec<&Value> = timing
            .iter()
            .take(case.prompt.len().div_ceil(a.chunk))
            .collect();
        let decode: Vec<&Value> = timing
            .iter()
            .skip(case.prompt.len().div_ceil(a.chunk))
            .collect();
        let sum = |v: &[&Value], key: &str| {
            v.iter()
                .map(|x| x[key].as_f64().unwrap_or(0.0))
                .sum::<f64>()
        };
        let decode_steady: Vec<&Value> = decode.iter().skip(1).copied().collect();
        let summary = json!({
            "prompt_tokens": case.prompt.len(),
            "prefill_seconds": sum(&prefill, "seconds"),
            "prefill_tokens_per_second": case.prompt.len() as f64 / sum(&prefill, "seconds"),
            "decode_steps": decode.len(),
            "decode_mean_seconds": sum(&decode_steady, "seconds") / decode_steady.len().max(1) as f64,
            "decode_mean_device_seconds": sum(&decode_steady, "device_seconds") / decode_steady.len().max(1) as f64,
            "decode_min_seconds": decode_steady.iter().map(|x| x["seconds"].as_f64().unwrap_or(0.0)).fold(f64::INFINITY, f64::min),
            "decode_max_seconds": decode_steady.iter().map(|x| x["seconds"].as_f64().unwrap_or(0.0)).fold(0.0, f64::max),
        });
        if a.timing_only {
            runs.push(json!({"prompt": id, "timing": summary, "steps": timing}));
            report["runs"] = json!(runs);
            write(&a.out, &report)?;
            continue;
        }
        // Observed run for the comparisons.
        let mut observed = Vec::new();
        for (position, tokens) in &steps {
            stage.step(&backend, &input(*position, tokens), true)?;
            observed.push((*position, tokens.len(), stage.observations(tokens.len())?));
        }
        let mut tally = Tally::new();
        for (k, (position, tokens, obs)) in observed.iter().enumerate() {
            let oracle_step = k.saturating_sub(case.prompt.len().div_ceil(a.chunk) - 1);
            let get =
                |layer: usize, site: V41Site, t: usize| gpu_site(obs, layer, site, t, *tokens);
            compare_with_oracle(
                &oracle,
                &case,
                &policy,
                *position,
                *tokens,
                oracle_step,
                &a.layers,
                streams,
                shape.attention.window,
                &get,
                &mut tally,
            )?;
        }
        fs::write(
            a.out.join(format!("gpu-oracle-{id}.json")),
            serde_json::to_vec_pretty(&tally.rows)?,
        )?;
        runs.push(json!({"prompt": id, "timing": summary, "steps": timing,
            "boundary_residual_bytes_per_step": boundary_bytes,
            "gpu_vs_oracle": tally.summary(), "memory_after": memory(&backend)}));
        report["runs"] = json!(runs);
        write(&a.out, &report)?;
        gpu_observed.insert(id.clone(), observed);
    }
    drop(stage);
    drop(backend);

    // ---------------- CPU ----------------
    if let Some(id) = &a.cpu {
        let case = Case::load(&a.oracle, id)?;
        let observed = gpu_observed
            .get(id)
            .ok_or("--cpu needs the prompt in --prompts")?;
        if a.layers.start != 0 {
            return Err("the CPU stage starts at layer 0".into());
        }
        let mut cpu = CpuStage::new(
            &source.0,
            BlockConfig::flash(),
            a.layers.end,
            layout.clone(),
            map.ids().to_vec(),
            pad,
            (shape.candidate_blocks, shape.candidate_block_size),
        )?;
        let mut versus = Vec::new();
        let mut oracle_tally = Tally::new();
        let t = Instant::now();
        for (k, (position, tokens, gpu)) in observed.iter().enumerate() {
            let step_tokens: Vec<u32> = case.steps(a.chunk)[k].1.clone();
            let out = cpu.step(&step_tokens)?;
            let mut layers = Vec::new();
            for layer in a.layers.clone() {
                let mut sites = Vec::new();
                for site in SITES {
                    let mut actual = Vec::new();
                    let mut expected = Vec::new();
                    for tk in 0..*tokens {
                        if let (Some(g), Some(c)) = (
                            gpu_site(gpu, layer, site, tk, *tokens),
                            cpu_site(&out.observations, layer, site, tk, *tokens),
                        ) {
                            let n = c.len().min(g.len());
                            actual.extend_from_slice(&g[..n]);
                            expected.extend_from_slice(&c[..n]);
                        }
                    }
                    if expected.is_empty() {
                        continue;
                    }
                    let fam = match site {
                        V41Site::AttnMixes | V41Site::FfnMixes | V41Site::RouteWeights => {
                            "float32_mixing_and_router"
                        }
                        V41Site::EngramIds | V41Site::ExpertIds | V41Site::Selection => {
                            "integer_and_hash"
                        }
                        _ => "bf16_activation_and_residual",
                    };
                    let (atol, rtol) = limits(&policy, fam);
                    let mut row = differences(&actual, &expected, atol, rtol);
                    row["site"] = json!(format!("{site:?}"));
                    sites.push(row);
                }
                layers.push(json!({"layer": layer, "sites": sites}));
            }
            let oracle_step = k.saturating_sub(case.prompt.len().div_ceil(a.chunk) - 1);
            let get = |layer: usize, site: V41Site, tk: usize| {
                cpu_site(&out.observations, layer, site, tk, *tokens)
            };
            compare_with_oracle(
                &oracle,
                &case,
                &policy,
                *position,
                *tokens,
                oracle_step,
                &a.layers,
                streams,
                shape.attention.window,
                &get,
                &mut oracle_tally,
            )?;
            versus
                .push(json!({"step": k, "position": position, "tokens": tokens, "layers": layers}));
            report["gpu_vs_cpu"] =
                json!({"prompt": id, "steps": versus, "seconds": t.elapsed().as_secs_f64()});
            write(&a.out, &report)?;
        }
        let failed: usize = versus
            .iter()
            .flat_map(|s| s["layers"].as_array().cloned().unwrap_or_default())
            .flat_map(|l| l["sites"].as_array().cloned().unwrap_or_default())
            .filter(|r| r["passed"] != true)
            .count();
        report["gpu_vs_cpu_failed_sites"] = json!(failed);
        report["cpu_vs_oracle"] = oracle_tally.summary();
        fs::write(
            a.out.join(format!("cpu-oracle-{id}.json")),
            serde_json::to_vec_pretty(&oracle_tally.rows)?,
        )?;
    }
    report["seconds"] = json!(started.elapsed().as_secs_f64());
    write(&a.out, &report)?;
    println!("{}", serde_json::to_string(&report["runs"])?);
    Ok(())
}

/// A first stage's step input: the tokens at `position`.
fn input(position: usize, tokens: &[u32]) -> V41StepInput<'_> {
    V41StepInput {
        position,
        tokens,
        residual: None,
        pre_mix: None,
        source_rows: None,
        selection: None,
        candidates: None,
        sampling: V41Sampling::greedy(),
    }
}
