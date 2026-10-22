//! Write exact-length chat prompts with the serving tokenizer and template.

use std::{error::Error, fs, path::PathBuf};

use psionic_models::deepseek_v41::DeepSeekV41Protocol;
use psionic_v41_stage::{PromptMode, Vocabulary};
use serde_json::json;
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("pass the tokenizer directory and output directory".into());
    }
    let source = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);
    let tokenizer = fs::read(source.join("tokenizer.json"))?;
    let config = fs::read(source.join("tokenizer_config.json"))?;
    let protocol = DeepSeekV41Protocol::from_checkpoint_files(&tokenizer, &config)?;
    let vocabulary = Vocabulary::V41(Box::new(protocol));
    fs::create_dir_all(&out)?;
    let mut records = Vec::new();
    for target in [2872, 2873] {
        let mut found = None;
        let (mut lo, mut hi) = (0, 4000);
        while lo <= hi {
            let n = (lo + hi) / 2;
            let text = format!(
                "The remaining words are filler. Reply with only OK, regardless of the filler.\n{}",
                " x".repeat(n)
            );
            let messages = json!([{"role":"user","content":text}]);
            let ids = vocabulary.encode(Some(&messages), 0, PromptMode::Chat)?;
            match ids.len().cmp(&target) {
                std::cmp::Ordering::Less => lo = n + 1,
                std::cmp::Ordering::Greater if n > 0 => hi = n - 1,
                std::cmp::Ordering::Greater => break,
                std::cmp::Ordering::Equal => {
                    found = Some((text, messages, ids));
                    break;
                }
            }
        }
        let (text, messages, ids) = found.ok_or("could not construct the exact token count")?;
        fs::write(out.join(format!("input-{target}.txt")), &text)?;
        fs::write(
            out.join(format!("input-{target}.json")),
            serde_json::to_vec_pretty(&messages)?,
        )?;
        records.push(json!({
            "input_tokens":ids.len(),
            "prompt_sha256":hex::encode(Sha256::digest(text.as_bytes())),
            "token_ids_sha256":hex::encode(Sha256::digest(
                ids.iter().flat_map(|id| id.to_le_bytes()).collect::<Vec<_>>()
            )),
        }));
    }
    let record = json!({
        "purpose":"exact input-boundary prompts using the native chat template",
        "mode":"chat",
        "tokenizer_sha256":hex::encode(Sha256::digest(&tokenizer)),
        "tokenizer_config_sha256":hex::encode(Sha256::digest(&config)),
        "prompts":records,
    });
    fs::write(out.join("inputs.json"), serde_json::to_vec_pretty(&record)?)?;
    println!("{record}");
    Ok(())
}
