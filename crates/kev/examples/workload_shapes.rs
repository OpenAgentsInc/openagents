//! Record encoded request sizes and exact-state digests without model inference.

use std::path::Path;

use gym::suite::{Partition, Suite};
use kev::{SystemOneRequest, encode, to_record};
use serde_json::json;
use sha2::{Digest, Sha256};
use tokenizers::Tokenizer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [adapter, suite_path] = args.as_slice() else {
        return Err("usage: workload_shapes ADAPTER SUITE".into());
    };
    let bytes = std::fs::read(Path::new(adapter).join("tokenizer.json"))?;
    let tokenizer = Tokenizer::from_bytes(&bytes).map_err(|e| e.to_string())?;
    let meta: serde_json::Value =
        serde_json::from_slice(&std::fs::read(Path::new(adapter).join("head_meta.json"))?)?;
    let isolation = meta["option_isolation"].as_bool().unwrap_or(false);
    let suite = Suite::load_file(suite_path)?;
    let questions = gym::questions::resolve(&suite, None)?;
    let mut rows = Vec::new();
    for partition in [Partition::Calibration, Partition::Development] {
        for item in suite.partition(partition)? {
            let question = questions.ask(item)?;
            let request: SystemOneRequest = serde_json::from_value(json!({
                "state": item.state, "questions": {"q": question},
            }))?;
            let (record, _) = to_record(&request)?;
            // Measure the packed size even when serving would refuse the branch.
            let enc = encode(&tokenizer, &record, 8192, usize::MAX, false, isolation)?;
            let state_tokens = enc.seg.iter().take_while(|seg| **seg == 0).count();
            let mut hash = Sha256::new();
            for id in &enc.ids[..state_tokens] {
                hash.update(id.to_le_bytes());
            }
            rows.push(json!({
                "item": item.id, "family": item.family, "partition": partition.as_str(),
                "state_sha256": format!("{:x}", hash.finalize()),
                "state_tokens": state_tokens, "packed_tokens": enc.ids.len(),
                "questions": record.questions.len(), "state_truncated": enc.state_truncated,
                "admitted_at_4096": enc.ids.len() <= 4096,
            }));
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "suite": suite.name, "suite_digest": suite.digest,
            "question_digest": questions.digest(),
            "tokenizer_sha256": format!("{:x}", Sha256::digest(&bytes)),
            "option_isolation": isolation, "max_state": 8192,
            "state_hash_encoding": "SHA-256 of little-endian u32 token IDs including the state delimiter",
            "rows": rows,
        }))?
    );
    Ok(())
}
