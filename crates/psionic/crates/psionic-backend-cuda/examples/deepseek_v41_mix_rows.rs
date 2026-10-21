//! Fetches the real DeepSeek-V4.1 weights and activations that the mixing,
//! Engram, and routing parity tests read (#12393).
//!
//! Every read is a ranged `gcloud storage cat -r`, so no shard is downloaded
//! whole. Shard headers and the index are parsed by the #12373 loader
//! (`psionic_models::deepseek_v41_weights`). Engram rows are read one at a
//! time at the oracle's own hash ids. Oracle tensors come from the #12371
//! dump and are checked against the SHA-256 values in its manifest.
//!
//! ```sh
//! CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config cargo run -p psionic-backend-cuda \
//!   --example deepseek_v41_mix_rows -- OUT_DIR
//! ```
//!
//! It writes `rows.json` and `rows.bin` to `OUT_DIR`.

use psionic_models::deepseek_v41_oracle::V41OracleFixtures;
use psionic_models::deepseek_v41_weights::{
    DEEPSEEK_V41_FLASH_INDEX_SHA256, DEEPSEEK_V41_FLASH_REVISION, SafetensorsIndex,
    SafetensorsShardHeader,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, path::PathBuf, process::Command};

const WEIGHTS: &str = "gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1";
const ORACLE: &str = "gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/dump";
const STEP: &str = "p1-chat-thinking.decode.first";

/// Whole checkpoint tensors: one attention and one FFN mixing projection,
/// one router, and the router biases and Engram gate weights the tests use.
const TENSORS: [&str; 16] = [
    "layers.0.hc_attn_fn",
    "layers.0.hc_attn_scale",
    "layers.0.hc_attn_base",
    "layers.39.hc_ffn_fn",
    "layers.39.hc_ffn_scale",
    "layers.39.hc_ffn_base",
    "layers.14.ffn.gate.weight",
    "layers.0.ffn.gate.bias",
    "layers.2.ffn.gate.bias",
    "layers.14.ffn.gate.bias",
    "layers.20.ffn.gate.bias",
    "layers.39.ffn.gate.bias",
    "layers.1.engram.q_weight",
    "layers.1.engram.k_weight",
    "layers.14.engram.q_weight",
    "layers.14.engram.k_weight",
];

const HC_LAYERS: [u32; 7] = [0, 1, 2, 14, 20, 21, 39];
const GATE_LAYERS: [u32; 5] = [0, 2, 14, 20, 39];
const ENGRAM_LAYERS: [u32; 2] = [1, 14];
const EXPERT_LAYERS: [u32; 2] = [2, 20];

fn oracle_sites() -> Vec<String> {
    let mut sites = Vec::new();
    for layer in HC_LAYERS {
        for site in [
            "hc.in",
            "hc.pre_mix_in",
            "attn.out",
            "hc.attn.pre",
            "hc.attn.post",
            "hc.attn.comb",
            "hc.attn.collapsed",
            "hc.attn.expanded",
            "hc.ffn.pre",
            "hc.ffn.post",
            "hc.ffn.comb",
            "hc.ffn.collapsed",
            "hc.ffn.expanded",
            "ffn.out",
        ] {
            sites.push(format!("layers.{layer}.{site}"));
        }
    }
    for layer in ENGRAM_LAYERS {
        for site in ["in", "out", "hash_ids", "rows"] {
            sites.push(format!("layers.{layer}.engram.{site}"));
        }
    }
    for layer in GATE_LAYERS {
        for site in ["in", "scores", "biased_scores", "ids", "weights"] {
            sites.push(format!("layers.{layer}.ffn.gate.{site}"));
        }
    }
    for layer in EXPERT_LAYERS {
        for site in ["id", "combine_weight", "w1_out", "w3_out", "w2_in"] {
            sites.push(format!("layers.{layer}.ffn.expert.{site}"));
        }
    }
    sites
}

fn cat(url: &str, range: Option<(u64, u64)>) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut command = Command::new("gcloud");
    command.args(["storage", "cat"]);
    if let Some((start, len)) = range {
        if len == 0 {
            return Ok(Vec::new());
        }
        command
            .arg("-r")
            .arg(format!("{start}-{}", start + len - 1));
    }
    let output = command.arg(url).output()?;
    if !output.status.success() {
        return Err(format!(
            "gcloud storage cat {url}: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    if let Some((_, len)) = range
        && output.stdout.len() as u64 != len
    {
        return Err(format!("{url}: wanted {len} bytes, got {}", output.stdout.len()).into());
    }
    Ok(output.stdout)
}

fn header_prefix(url: &str) -> Result<(u64, Vec<u8>), Box<dyn Error>> {
    let first = cat(url, Some((0, 8)))?;
    let len = u64::from_le_bytes(first.as_slice().try_into()?);
    let mut prefix = first;
    prefix.extend(cat(url, Some((8, len)))?);
    Ok((len, prefix))
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

struct Blob(Vec<u8>);

impl Blob {
    fn push(
        &mut self,
        label: &str,
        source: &str,
        dtype: &str,
        shape: Value,
        bytes: &[u8],
    ) -> Value {
        let offset = self.0.len();
        self.0.extend_from_slice(bytes);
        // Keep every section 16-byte aligned for vector loads after upload.
        while !self.0.len().is_multiple_of(16) {
            self.0.push(0);
        }
        json!({
            "label": label, "source": source, "dtype": dtype, "shape": shape,
            "offset": offset, "bytes": bytes.len(), "sha256": sha256(bytes),
        })
    }
}

struct Shards {
    index: SafetensorsIndex,
    headers: BTreeMap<String, SafetensorsShardHeader>,
}

impl Shards {
    fn entry(
        &mut self,
        tensor: &str,
    ) -> Result<
        (
            String,
            psionic_models::deepseek_v41_weights::ShardTensorEntry,
        ),
        Box<dyn Error>,
    > {
        let shard = self
            .index
            .weight_map
            .get(tensor)
            .ok_or_else(|| format!("{tensor} is not in the index"))?
            .clone();
        if !self.headers.contains_key(&shard) {
            let (_, prefix) = header_prefix(&format!("{WEIGHTS}/{shard}"))?;
            self.headers.insert(
                shard.clone(),
                SafetensorsShardHeader::parse(&shard, &prefix, None)?,
            );
        }
        let entry = self.headers[&shard]
            .tensors
            .get(tensor)
            .ok_or_else(|| format!("{tensor} missing from {shard}"))?
            .clone();
        Ok((format!("{WEIGHTS}/{shard}"), entry))
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: deepseek_v41_mix_rows OUT_DIR")?,
    );
    std::fs::create_dir_all(&out)?;
    let index = SafetensorsIndex::parse(&cat(
        &format!("{WEIGHTS}/model.safetensors.index.json"),
        None,
    )?)?;
    if index.sha256 != DEEPSEEK_V41_FLASH_INDEX_SHA256 {
        return Err("the bucket's index doesn't match the pinned checkpoint".into());
    }
    let oracle = V41OracleFixtures::load_checked_in()?;
    let mut shards = Shards {
        index,
        headers: BTreeMap::new(),
    };
    let mut blob = Blob(Vec::new());
    let mut sections = Vec::new();

    // Oracle tensors.
    let mut oracle_headers: BTreeMap<String, (u64, Value)> = BTreeMap::new();
    let mut hash_ids: BTreeMap<u32, Vec<u64>> = BTreeMap::new();
    for site in oracle_sites() {
        let name = format!("{STEP}.{site}");
        let Ok(tensor) = oracle.tensor(&name) else {
            eprintln!("skip {name}: not recorded");
            continue;
        };
        let url = format!("{ORACLE}/{}", tensor.file);
        if !oracle_headers.contains_key(&tensor.file) {
            let (len, prefix) = header_prefix(&url)?;
            oracle_headers.insert(
                tensor.file.clone(),
                (len, serde_json::from_slice(&prefix[8..])?),
            );
        }
        let (len, parsed) = &oracle_headers[&tensor.file];
        let entry = &parsed[&name];
        let start = entry["data_offsets"][0]
            .as_u64()
            .ok_or("bad data offsets")?;
        let end = entry["data_offsets"][1]
            .as_u64()
            .ok_or("bad data offsets")?;
        let bytes = cat(&url, Some((8 + len + start, end - start)))?;
        if !oracle.matches(&name, &bytes)? {
            return Err(format!("{name}: SHA-256 doesn't match the oracle manifest").into());
        }
        if let Some(layer) = site
            .strip_suffix(".engram.hash_ids")
            .and_then(|l| l.strip_prefix("layers."))
        {
            hash_ids.insert(
                layer.parse()?,
                bytes
                    .chunks_exact(8)
                    .map(|c| u64::from_le_bytes(c.try_into().expect("8 bytes")))
                    .collect(),
            );
        }
        sections.push(blob.push(&site, &name, &tensor.dtype, json!(tensor.shape), &bytes));
        eprintln!("{site}");
    }

    // Whole checkpoint tensors.
    for tensor in TENSORS {
        let (url, entry) = shards.entry(tensor)?;
        let bytes = cat(&url, Some((entry.file_offset, entry.byte_len)))?;
        sections.push(blob.push(
            tensor,
            tensor,
            &format!("{:?}", entry.dtype),
            json!(entry.shape),
            &bytes,
        ));
        eprintln!("{tensor}: {} bytes", bytes.len());
    }

    // Engram rows at the oracle's hash ids, one ranged read per row.
    for layer in ENGRAM_LAYERS {
        let ids = &hash_ids[&layer];
        let (url, codes) = shards.entry(&format!("layers.{layer}.engram.embed.weight"))?;
        let (scale_url, scales) = shards.entry(&format!("layers.{layer}.engram.embed.scale"))?;
        let (rows, width) = (codes.shape[0], codes.shape[1]);
        let groups = scales.shape[1];
        if scales.shape[0] != rows || groups * 32 != width {
            return Err(format!("layer {layer}: unexpected Engram table shapes").into());
        }
        let mut code_bytes = Vec::new();
        let mut scale_bytes = Vec::new();
        for id in ids {
            if *id >= rows {
                return Err(format!("layer {layer}: row {id} outside {rows}").into());
            }
            code_bytes.extend(cat(&url, Some((codes.file_offset + id * width, width)))?);
            scale_bytes.extend(cat(
                &scale_url,
                Some((scales.file_offset + id * groups, groups)),
            )?);
        }
        let source = format!("layers.{layer}.engram.embed rows at the oracle's hash ids");
        sections.push(blob.push(
            &format!("layers.{layer}.engram.embed.weight.rows"),
            &source,
            "F8E4M3",
            json!([ids.len(), width]),
            &code_bytes,
        ));
        sections.push(blob.push(
            &format!("layers.{layer}.engram.embed.scale.rows"),
            &source,
            "F8E8M0",
            json!([ids.len(), groups]),
            &scale_bytes,
        ));
        eprintln!("layer {layer}: {} Engram rows of {rows}", ids.len());
    }

    let manifest = json!({
        "schema": "psionic.deepseek_v41.mix_rows.v1",
        "revision": DEEPSEEK_V41_FLASH_REVISION,
        "weights": WEIGHTS,
        "oracle": ORACLE,
        "step": STEP,
        "blob": "rows.bin",
        "blob_sha256": sha256(&blob.0),
        "sections": sections,
    });
    std::fs::write(out.join("rows.bin"), &blob.0)?;
    std::fs::write(
        out.join("rows.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    eprintln!("wrote {} bytes", blob.0.len());
    Ok(())
}
