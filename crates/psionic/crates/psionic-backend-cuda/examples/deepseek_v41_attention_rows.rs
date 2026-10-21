//! Fetches the real DeepSeek-V4.1 weights and activations that the attention
//! parity tests read (#12392).
//!
//! Every read is a ranged `gcloud storage cat -r`, so no shard is downloaded
//! whole. Shard headers and the index are parsed by the #12373 loader
//! (`psionic_models::deepseek_v41_weights`). Oracle tensors come from the
//! #12371 dump and are checked against the SHA-256 values in the bucket's
//! manifest, and, for the four main prompts, in the checked-in manifest too.
//!
//! ```sh
//! CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config cargo run -p psionic-backend-cuda \
//!   --example deepseek_v41_attention_rows -- OUT_DIR
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

/// Whole checkpoint tensors: the compressors and indexer keys of a ratio-2
/// source (layer 2) and the ratio-1 source (layer 20), their index weight
/// projections, and three layers' attention sinks.
const TENSORS: [&str; 15] = [
    "layers.2.attn.compressor.wkv.weight",
    "layers.2.attn.compressor.wgate.weight",
    "layers.2.attn.compressor.norm.weight",
    "layers.2.attn.indexer.wk.weight",
    "layers.2.attn.indexer.k_norm.weight",
    "layers.2.attn.indexer.weights_proj.weight",
    "layers.20.attn.compressor.wkv.weight",
    "layers.20.attn.compressor.norm.weight",
    "layers.20.attn.indexer.wk.weight",
    "layers.20.attn.indexer.k_norm.weight",
    "layers.20.attn.indexer.weights_proj.weight",
    "layers.0.attn.attn_sink",
    "layers.2.attn.attn_sink",
    "layers.20.attn.attn_sink",
    "layers.24.attn.attn_sink",
];

/// Prompts and the decode-mode steps the tests read.
const PROMPTS: [(&str, &[&str]); 7] = [
    ("p1-chat-thinking", &["last", "g1", "g2", "g32"]),
    ("p2-code", &["last", "g1", "g2", "g32"]),
    ("p3-2k", &["last", "g1", "g2", "g32"]),
    ("p4-20k", &["last", "g1", "g2", "g32"]),
    ("e-window-127", &["last", "g1", "g2", "g8"]),
    ("e-window-128", &["last", "g1", "g2", "g8"]),
    ("e-window-129", &["last", "g1", "g2", "g8"]),
];

const INDEX_SOURCES: [u32; 8] = [2, 8, 14, 20, 24, 28, 32, 36];

fn sites() -> Vec<String> {
    let mut sites = Vec::new();
    for layer in [2, 20] {
        sites.push(format!("layers.{layer}.attn.in"));
        sites.push(format!("layers.{layer}.attn.compressor.latent"));
    }
    for layer in INDEX_SOURCES {
        sites.push(format!("layers.{layer}.attn.indexer.score"));
        sites.push(format!("layers.{layer}.attn.indexer.topk_idxs"));
    }
    sites.push(String::from("layers.20.attn.indexer.candidate_blocks"));
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

fn main() -> Result<(), Box<dyn Error>> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: deepseek_v41_attention_rows OUT_DIR")?,
    );
    std::fs::create_dir_all(&out)?;
    let index = SafetensorsIndex::parse(&cat(
        &format!("{WEIGHTS}/model.safetensors.index.json"),
        None,
    )?)?;
    if index.sha256 != DEEPSEEK_V41_FLASH_INDEX_SHA256 {
        return Err("the bucket's index doesn't match the pinned checkpoint".into());
    }
    let checked_in = V41OracleFixtures::load_checked_in()?;
    let bucket: Value = serde_json::from_slice(&cat(&format!("{ORACLE}/manifest.json"), None)?)?;
    let bucket_tensors: BTreeMap<String, Value> = bucket["tensors"]
        .as_array()
        .ok_or("bucket manifest has no tensors")?
        .iter()
        .filter_map(|t| Some((t["name"].as_str()?.to_owned(), t.clone())))
        .collect();
    let mut blob = Blob(Vec::new());
    let mut sections = Vec::new();

    // Oracle tensors.
    let mut oracle_headers: BTreeMap<String, (u64, Value)> = BTreeMap::new();
    for (prompt, steps) in PROMPTS {
        for step in steps {
            for site in sites() {
                let name = format!("{prompt}.decode.{step}.{site}");
                let Some(tensor) = bucket_tensors.get(&name) else {
                    continue;
                };
                let file = tensor["file"].as_str().ok_or("tensor without a file")?;
                let url = format!("{ORACLE}/{file}");
                if !oracle_headers.contains_key(file) {
                    let (len, prefix) = header_prefix(&url)?;
                    oracle_headers.insert(
                        file.to_owned(),
                        (len, serde_json::from_slice(&prefix[8..])?),
                    );
                }
                let (len, parsed) = &oracle_headers[file];
                let entry = &parsed[&name];
                let start = entry["data_offsets"][0]
                    .as_u64()
                    .ok_or("bad data offsets")?;
                let end = entry["data_offsets"][1]
                    .as_u64()
                    .ok_or("bad data offsets")?;
                let bytes = cat(&url, Some((8 + len + start, end - start)))?;
                if Some(sha256(&bytes).as_str()) != tensor["sha256"].as_str() {
                    return Err(format!("{name}: SHA-256 doesn't match the bucket manifest").into());
                }
                if checked_in.tensor(&name).is_ok() && !checked_in.matches(&name, &bytes)? {
                    return Err(
                        format!("{name}: SHA-256 doesn't match the checked-in manifest").into(),
                    );
                }
                let dtype = tensor["dtype"].as_str().ok_or("tensor without a dtype")?;
                sections.push(blob.push(&name, &name, dtype, tensor["shape"].clone(), &bytes));
            }
            eprintln!("{prompt} {step}");
        }
    }

    // Whole checkpoint tensors.
    let mut headers: BTreeMap<String, SafetensorsShardHeader> = BTreeMap::new();
    for tensor in TENSORS {
        let shard = index
            .weight_map
            .get(tensor)
            .ok_or_else(|| format!("{tensor} is not in the index"))?
            .clone();
        if !headers.contains_key(&shard) {
            let (_, prefix) = header_prefix(&format!("{WEIGHTS}/{shard}"))?;
            headers.insert(
                shard.clone(),
                SafetensorsShardHeader::parse(&shard, &prefix, None)?,
            );
        }
        let entry = headers[&shard]
            .tensors
            .get(tensor)
            .ok_or_else(|| format!("{tensor} missing from {shard}"))?
            .clone();
        let bytes = cat(
            &format!("{WEIGHTS}/{shard}"),
            Some((entry.file_offset, entry.byte_len)),
        )?;
        sections.push(blob.push(
            tensor,
            tensor,
            &format!("{:?}", entry.dtype),
            json!(entry.shape),
            &bytes,
        ));
        eprintln!("{tensor}: {} bytes", bytes.len());
    }

    let manifest = json!({
        "schema": "psionic.deepseek_v41.attention_rows.v1",
        "revision": DEEPSEEK_V41_FLASH_REVISION,
        "weights": WEIGHTS,
        "oracle": ORACLE,
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
