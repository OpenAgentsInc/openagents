//! Check the pinned R1 GGUF's byte identity, metadata, tokenizer, and prompts.

use std::{env, fs::File, io::Read, path::Path};

use psionic_models::{
    DEEPSEEK_R1_DISTILL_QWEN_1_5B_GGUF_SHA256, GgufContent,
    qualify_deepseek_r1_distill_qwen_1_5b_text,
};
use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("Pass the pinned R1 Q4_K_M GGUF path.")?;
    if args.next().is_some() {
        return Err("Pass exactly one GGUF path.".into());
    }
    let mut file = File::open(&path)?;
    if file.metadata()?.len() != 1_117_321_312 {
        return Err("The GGUF has the wrong byte length.".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    let digest = hex::encode(hash.finalize());
    if digest != DEEPSEEK_R1_DISTILL_QWEN_1_5B_GGUF_SHA256 {
        return Err("The GGUF differs from the pinned SHA-256.".into());
    }
    let report =
        qualify_deepseek_r1_distill_qwen_1_5b_text(&GgufContent::read_path(Path::new(&path))?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "artifact_sha256": digest,
            "text_qualification": report,
            "numerical_execution": "not_checked",
            "gpu_execution": "not_checked"
        }))?
    );
    Ok(())
}
