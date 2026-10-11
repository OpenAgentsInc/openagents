//! Export text IDs and spans with the production encoder, without loading weights.
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

#[path = "production-source/clef/mod.rs"]
mod clef;

use clef::encode::{ClefRequest, RequestLimits, encode_record};
use clef::json as clef_json;
use psionic_models::{GgufContent, GgufRuntimeTokenizer};
use serde_json::json;
use sha2::{Digest, Sha256};

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        return Err("usage: encoder GGUF REQUEST_DIRECTORY OUTPUT_JSON".into());
    }
    let resources = std::env::var("OPENAGENTS_LEASES").unwrap_or_default();
    let leases: Vec<&str> = resources.split(',').collect();
    if !leases.contains(&"memory")
        || leases
            .iter()
            .any(|name| ["gpu", "quiet", "build"].contains(name))
        || std::env::var("CLEF_ENCODER_ONLY_GUARDED").as_deref() != Ok("1")
    {
        return Err("requires the bounded CPU-only encoder guard".into());
    }
    let gguf = Path::new(&args[1]);
    if fs::metadata(gguf).map_err(|e| e.to_string())?.len() != 19_232_219_200 {
        return Err("GGUF size differs from the pinned 27B artifact".into());
    }
    if Path::new(&args[3]).exists() {
        return Err("output already exists".into());
    }
    let content = GgufContent::read_path(gguf).map_err(|e| e.to_string())?;
    let tokenizer =
        GgufRuntimeTokenizer::from_gguf(&content.load_tokenizer().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let tokenize = |text: &str| {
        tokenizer
            .encode_with_special_tokens(text, false, false)
            .as_slice()
            .iter()
            .map(|token| token.as_u32())
            .collect::<Vec<u32>>()
    };
    let mut cases = Vec::new();
    let mut questions = 0;
    let mut tokens = 0;
    for index in 0..40 {
        let id = format!("text-{index:03}");
        let path = Path::new(&args[2]).join(format!("{id}.request.json"));
        if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 65_536 {
            return Err(format!("{id}: request exceeds byte bound"));
        }
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let body = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
        let value = clef_json::parse(body).map_err(|e| e.to_string())?;
        let request = ClefRequest::from_json(
            &value,
            RequestLimits {
                max_questions: 64,
                max_options: 255,
            },
        )
        .map_err(|e| format!("{id}: {e:?}"))?;
        let record =
            encode_record(&request, &tokenize, 16_384).map_err(|e| format!("{id}: {e:?}"))?;
        if record.media.is_some()
            || record.truncated_state_tokens != 0
            || record.input_ids.is_empty()
            || record.input_ids.len() > 2_048
        {
            return Err(format!(
                "{id}: unexpected media, truncation, or token count"
            ));
        }
        questions += record.questions.len();
        tokens += record.input_ids.len();
        cases.push(json!({
            "id": id,
            "request_sha256": digest(&bytes),
            "input_ids": record.input_ids,
            "prefix_tokens": record.prefix_tokens,
            "truncated_state_tokens": record.truncated_state_tokens,
            "questions": record.questions.iter().map(|q| json!({
                "id": q.question_id,
                "type": q.question_type,
                "question_span": [q.question_span.0, q.question_span.1],
                "option_spans": q.option_spans.iter().map(|(a,b)| [*a,*b]).collect::<Vec<_>>(),
                "option_ids": q.option_ids,
            })).collect::<Vec<_>>(),
        }));
    }
    if questions != 104 || tokens != 20_894 {
        return Err(format!(
            "suite counts differ: {questions} questions, {tokens} tokens"
        ));
    }
    let result = json!({
        "schema": "clef.native-encoder-only.v1",
        "status": "complete",
        "model_executed": false,
        "gpu_executed": false,
        "weight_payload_decoded": false,
        "gguf_read_policy": "Production bounded header reader; its geometric read may include adjacent payload bytes, but no weight tensor is decoded or executed.",
        "gguf_sha256_expected_from_guard": "9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822",
        "cases": cases,
        "question_count": questions,
        "total_tokens": tokens,
    });
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[3])
        .map_err(|e| e.to_string())?;
    output
        .write_all(&serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    output.write_all(b"\n").map_err(|e| e.to_string())?;
    println!("exported 40 requests, 104 questions, and {tokens} tokens");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
