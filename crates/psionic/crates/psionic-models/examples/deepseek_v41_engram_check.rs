//! Check the pinned tokenizer's Engram map without downloading model weights.

use psionic_models::{
    deepseek_v41::{DEEPSEEK_V41_FLASH_REVISION, DEEPSEEK_V41_TOKENIZER_SHA256},
    deepseek_v41_cpu::engram::{
        COMPRESSED_VOCAB_SIZE, CompressedTokenMap, EngramLayout, NgramHashState,
    },
};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, sync::Arc};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: deepseek_v41_engram_check TOKENIZER_JSON")?;
    let bytes = fs::read(path)?;
    let tokenizer_sha256 = hex::encode(Sha256::digest(&bytes));
    if tokenizer_sha256 != DEEPSEEK_V41_TOKENIZER_SHA256 {
        return Err("tokenizer SHA-256 does not match the pinned checkpoint".into());
    }
    let tokenizer = tokenizers::Tokenizer::from_bytes(bytes).map_err(|e| e.to_string())?;
    let map = CompressedTokenMap::build(&tokenizer, COMPRESSED_VOCAB_SIZE)?;
    let mut digest = Sha256::new();
    for id in map.ids() {
        digest.update(id.to_le_bytes());
    }
    let layout = Arc::new(EngramLayout::flash()?);
    let mut state = NgramHashState::new(layout.clone(), map.get(2)?)?;
    let token_ids = tokenizer
        .encode(" The thé THE Rust\nGPU", false)
        .map_err(|e| e.to_string())?
        .get_ids()
        .to_vec();
    let hashes = state.push_tokens(0, &token_ids, None, &map)?;
    let report = serde_json::json!({
        "schema": "psionic.deepseek_v41.engram_tokenizer.v1",
        "revision": DEEPSEEK_V41_FLASH_REVISION,
        "tokenizer_sha256": tokenizer_sha256,
        "token_count": map.ids().len(), "compressed_vocab_size": map.vocab_size(),
        "compressed_pad_id": map.get(2)?,
        "map_u32_le_sha256": hex::encode(digest.finalize()),
        "table_rows": layout.layers.iter().map(|l| l.table_rows).collect::<Vec<_>>(),
        "probe_token_ids": token_ids,
        "probe_hashes": hashes.iter().map(|h| &h.layers).collect::<Vec<_>>(),
        "weight_bytes_opened": 0, "device_oracle_compared": false
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
