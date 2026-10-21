//! Fetches the real DeepSeek-V4.1 rows and activations that the block-scaled
//! matrix-vector parity tests read (#12391).
//!
//! Every read is a ranged `gcloud storage cat -r`, so no shard is downloaded
//! whole. Shard headers and the index are parsed by the #12373 loader
//! (`psionic_models::deepseek_v41_weights`), and each fetched row is decoded
//! with its dequantization functions before it is written. Oracle vectors are
//! checked against the SHA-256 values in the #12371 oracle manifest.
//!
//! ```sh
//! CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config cargo run -p psionic-backend-cuda \
//!   --example deepseek_v41_matvec_rows -- OUT_DIR
//! ```
//!
//! It writes `rows.json` and `rows.bin` to `OUT_DIR`.

use psionic_models::deepseek_v41_weights::{
    DEEPSEEK_V41_FLASH_INDEX_SHA256, DEEPSEEK_V41_FLASH_REVISION, SafetensorsDtype,
    SafetensorsIndex, SafetensorsShardHeader, V41_SCALE_BLOCK, dequantize_fp4_row,
    dequantize_fp8_per32,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{error::Error, path::PathBuf, process::Command};

const WEIGHTS: &str = "gs://openagents-psionic-models/deepseek-ai/DeepSeek-V4.1-Flash-1017/1bae0e1";
const ORACLE: &str = "gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/dump";

/// `(label, tensor, first row, row count)`. Each range is one whole 32-row
/// block so FP8 block scales stay aligned with the fetched rows.
const MATRICES: [(&str, &str, u64, u64); 5] = [
    ("wq_a", "layers.2.attn.wq_a.weight", 1248, 32),
    ("shared_w1", "layers.2.ffn.shared_experts.w1.weight", 0, 32),
    (
        "shared_w2",
        "layers.2.ffn.shared_experts.w2.weight",
        5088,
        32,
    ),
    ("expert_w1", "layers.2.ffn.experts.222.w1.weight", 1120, 32),
    ("expert_w2", "layers.2.ffn.experts.222.w2.weight", 2048, 32),
];

/// `(label, oracle file, tensor, SHA-256 from the oracle manifest)`. The
/// chat prompt's first decode step routes layer 2 to expert 222.
const VECTORS: [(&str, &str, &str, &str); 9] = [
    (
        "attn_in",
        "p1-chat-thinking/tensors-rank0.safetensors",
        "p1-chat-thinking.decode.first.layers.2.attn.in",
        "7a0227c88c2d4df0b6d4836cadc3a8272975dbaf101c176fd4ae52b8c9ba16da",
    ),
    (
        "ffn_in",
        "p1-chat-thinking/tensors-rank4.safetensors",
        "p1-chat-thinking.decode.first.layers.2.ffn.expert.in",
        "51d93d14a62e66d743910733f8fc55feb0beba24ef7a22723a262203478a3618",
    ),
    (
        "ffn_in_fp8",
        "p1-chat-thinking/tensors-rank4.safetensors",
        "p1-chat-thinking.decode.first.layers.2.ffn.expert.in_fp8",
        "1a02d35b19afb4407cb3febefada3f4184bd4c1a0de53cad3ab33fe65eb4e41b",
    ),
    (
        "ffn_in_fp8_scale",
        "p1-chat-thinking/tensors-rank4.safetensors",
        "p1-chat-thinking.decode.first.layers.2.ffn.expert.in_fp8_scale",
        "14a292a2dd31e75955aeabb6e9a6f15b3f0ccbea2e38774b8d10fa6b88513f2d",
    ),
    (
        "expert_w1_out",
        "p1-chat-thinking/tensors-rank4.safetensors",
        "p1-chat-thinking.decode.first.layers.2.ffn.expert.w1_out",
        "6b6383bf2d1b704af2c8a8db9590be02d1cd94367b877b9a50fa46ae2f0c4fc3",
    ),
    (
        "w2_in",
        "p1-chat-thinking/tensors-rank4.safetensors",
        "p1-chat-thinking.decode.first.layers.2.ffn.expert.w2_in",
        "10f2035b52be39afeb151c961187af433cb61137b063d0d5b66c7f000bfdfa53",
    ),
    (
        "w2_in_fp8",
        "p1-chat-thinking/tensors-rank4.safetensors",
        "p1-chat-thinking.decode.first.layers.2.ffn.expert.w2_in_fp8",
        "1b4a469befd473dab3de0bc8f31396fca6f7144a66052eaf46d64eb0239c5e93",
    ),
    (
        "w2_in_fp8_scale",
        "p1-chat-thinking/tensors-rank4.safetensors",
        "p1-chat-thinking.decode.first.layers.2.ffn.expert.w2_in_fp8_scale",
        "51232fdd84e5d97c452acb141614c801b060b770545dbd787a8a62d628ed6742",
    ),
    (
        "expert_w2_out",
        "p1-chat-thinking/tensors-rank4.safetensors",
        "p1-chat-thinking.decode.first.layers.2.ffn.expert.w2_out",
        "2f3c536f2226eef63a0f6f1ee88d392a32f0cb3f8c846b310a086c901ce3d424",
    ),
];

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
    fn push(&mut self, bytes: &[u8]) -> Value {
        let offset = self.0.len();
        self.0.extend_from_slice(bytes);
        // Keep every section 16-byte aligned for vector loads after upload.
        while !self.0.len().is_multiple_of(16) {
            self.0.push(0);
        }
        json!({"offset": offset, "bytes": bytes.len(), "sha256": sha256(bytes)})
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: deepseek_v41_matvec_rows OUT_DIR")?,
    );
    std::fs::create_dir_all(&out)?;
    let index = SafetensorsIndex::parse(&cat(
        &format!("{WEIGHTS}/model.safetensors.index.json"),
        None,
    )?)?;
    if index.sha256 != DEEPSEEK_V41_FLASH_INDEX_SHA256 {
        return Err("the bucket's index doesn't match the pinned checkpoint".into());
    }
    let mut blob = Blob(Vec::new());
    let mut matrices = Vec::new();
    for (label, tensor, first, count) in MATRICES {
        let shard = index
            .weight_map
            .get(tensor)
            .ok_or_else(|| format!("{tensor} is not in the index"))?
            .clone();
        let url = format!("{WEIGHTS}/{shard}");
        let (_, prefix) = header_prefix(&url)?;
        let header = SafetensorsShardHeader::parse(&shard, &prefix, None)?;
        let weight = header
            .tensors
            .get(tensor)
            .ok_or_else(|| format!("{tensor} missing from {shard}"))?;
        let scale_name = tensor.replace(".weight", ".scale");
        let scale = header
            .tensors
            .get(&scale_name)
            .ok_or_else(|| format!("{scale_name} missing from {shard}"))?;
        let (rows, stored_cols) = (weight.shape[0], weight.shape[1]);
        let fp4 = weight.dtype == SafetensorsDtype::I8;
        let cols = if fp4 { stored_cols * 2 } else { stored_cols };
        let block = V41_SCALE_BLOCK as u64;
        if first % block != 0 || first + count > rows || cols % block != 0 {
            return Err(format!("{tensor}: rows {first}+{count} don't fit {rows}x{cols}").into());
        }
        let row_bytes = stored_cols;
        let weight_bytes = cat(
            &url,
            Some((weight.file_offset + first * row_bytes, count * row_bytes)),
        )?;
        let scale_cols = cols / block;
        // FP4 has one scale row per weight row; FP8 has one per 32 rows.
        let (scale_first, scale_rows) = if fp4 {
            (first, count)
        } else {
            (first / block, count.div_ceil(block))
        };
        let scale_bytes = cat(
            &url,
            Some((
                scale.file_offset + scale_first * scale_cols,
                scale_rows * scale_cols,
            )),
        )?;
        for row in 0..count as usize {
            let bytes = &weight_bytes[row * row_bytes as usize..(row + 1) * row_bytes as usize];
            let values = if fp4 {
                let scales =
                    &scale_bytes[row * scale_cols as usize..(row + 1) * scale_cols as usize];
                dequantize_fp4_row(bytes, scales)?
            } else {
                let block_row = row / V41_SCALE_BLOCK;
                let scales = &scale_bytes
                    [block_row * scale_cols as usize..(block_row + 1) * scale_cols as usize];
                dequantize_fp8_per32(bytes, scales)?
            };
            if values.len() as u64 != cols || values.iter().any(|v| !v.is_finite()) {
                return Err(format!("{tensor} row {} doesn't decode", first + row as u64).into());
            }
        }
        matrices.push(json!({
            "label": label,
            "tensor": tensor,
            "shard": shard,
            "format": if fp4 { "fp4_e2m1_row32" } else { "fp8_block32x32" },
            "shape": [rows, cols],
            "first_row": first,
            "rows": count,
            "weight_file_offset": weight.file_offset + first * row_bytes,
            "scale_file_offset": scale.file_offset + scale_first * scale_cols,
            "weight": blob.push(&weight_bytes),
            "scale": blob.push(&scale_bytes),
        }));
        eprintln!(
            "{label}: {tensor} rows {first}..{} from {shard}",
            first + count
        );
    }
    let mut vectors = Vec::new();
    let mut headers = std::collections::BTreeMap::new();
    for (label, file, tensor, expected) in VECTORS {
        let url = format!("{ORACLE}/{file}");
        if !headers.contains_key(file) {
            let (len, prefix) = header_prefix(&url)?;
            let parsed: Value = serde_json::from_slice(&prefix[8..])?;
            headers.insert(file, (len, parsed));
        }
        let (len, parsed) = &headers[file];
        let entry = &parsed[tensor];
        let start = entry["data_offsets"][0]
            .as_u64()
            .ok_or("bad data offsets")?;
        let end = entry["data_offsets"][1]
            .as_u64()
            .ok_or("bad data offsets")?;
        let bytes = cat(&url, Some((8 + len + start, end - start)))?;
        if sha256(&bytes) != expected {
            return Err(format!("{tensor}: SHA-256 doesn't match the oracle manifest").into());
        }
        let mut record = blob.push(&bytes);
        record["label"] = json!(label);
        record["tensor"] = json!(tensor);
        record["dtype"] = entry["dtype"].clone();
        record["shape"] = entry["shape"].clone();
        vectors.push(record);
        eprintln!("{label}: {tensor}");
    }
    let manifest = json!({
        "schema": "psionic.deepseek_v41.matvec_rows.v1",
        "revision": DEEPSEEK_V41_FLASH_REVISION,
        "weights": WEIGHTS,
        "oracle": ORACLE,
        "blob": "rows.bin",
        "blob_sha256": sha256(&blob.0),
        "matrices": matrices,
        "vectors": vectors,
    });
    std::fs::write(out.join("rows.bin"), &blob.0)?;
    std::fs::write(
        out.join("rows.json"),
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?;
    eprintln!("wrote {} bytes of rows", blob.0.len());
    Ok(())
}
