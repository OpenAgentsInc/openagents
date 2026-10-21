//! Measure and check Flint's Metal path offline.
//!
//! `generate --weights DIR --request FILE [--context N] [--runs N]`
//!   renders the request's messages, generates, and prints token IDs and timing.
//! `logits --weights DIR --request FILE` prints one JSON line of logits per
//!   position of the request's `token_ids`, in `psionic-flint logits` format.
use std::path::PathBuf;

use anyhow::{Context, Result, bail, ensure};
use psionic_flint::{
    Message, Sampling, load_tokenizer,
    metal::{MetalModel, generate},
    render,
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    messages: Vec<Message>,
    #[serde(default)]
    token_ids: Vec<u32>,
    #[serde(default = "default_tokens")]
    max_tokens: usize,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    top_p: Option<f64>,
    #[serde(default)]
    top_k: Option<usize>,
    #[serde(default)]
    seed: Option<u64>,
}
fn default_tokens() -> usize {
    256
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_default();
    let mut weights = None;
    let mut request = None;
    let mut context = 32768;
    let mut runs = 1;
    let mut verify = false;
    while let Some(key) = args.next() {
        if key == "--verify" {
            verify = true;
            continue;
        }
        let value = args.next().context("missing option value")?;
        match key.as_str() {
            "--weights" => weights = Some(PathBuf::from(value)),
            "--request" => request = Some(PathBuf::from(value)),
            "--context" => context = value.parse()?,
            "--runs" => runs = value.parse()?,
            _ => bail!("unknown option {key}"),
        }
    }
    let root = weights.context("--weights is required")?;
    let request: Request =
        serde_json::from_slice(&std::fs::read(request.context("--request is required")?)?)?;
    let tokenizer = load_tokenizer(&root.join("tokenizer.json"))?;
    let model = MetalModel::load(&root, verify)?;
    eprintln!(
        "loaded {:.1} GB on {} in {:.1} s",
        model.weight_bytes() as f64 / 1e9,
        model.device_name(),
        model.load_seconds
    );
    match mode.as_str() {
        "logits" => {
            ensure!(!request.token_ids.is_empty(), "token_ids is required");
            let mut session = model.session(context)?;
            let mut logits = Vec::new();
            for (position, token) in request.token_ids.iter().enumerate() {
                if position == 0 {
                    model.prefill(&mut session, &[*token])?;
                } else {
                    model.decode(&mut session, *token)?;
                }
                model.logits(&mut logits);
                println!(
                    "{}",
                    json!({"position": position, "token": token, "logits": logits})
                );
            }
        }
        "generate" => {
            let prompt: Vec<u32> = if request.token_ids.is_empty() {
                let text = render(&request.messages)?;
                tokenizer
                    .encode(text, false)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?
                    .get_ids()
                    .to_vec()
            } else {
                request.token_ids.clone()
            };
            let mut sampling = Sampling {
                temperature: 0.0,
                ..Sampling::default()
            };
            if let Some(v) = request.temperature {
                sampling.temperature = v;
            }
            if let Some(v) = request.top_p {
                sampling.top_p = v;
            }
            if let Some(v) = request.top_k {
                sampling.top_k = v;
            }
            if let Some(v) = request.seed {
                sampling.seed = v;
            }
            for run in 0..runs {
                let (generation, timing) = generate(
                    &model,
                    &tokenizer,
                    &prompt,
                    request.max_tokens,
                    context,
                    &sampling,
                    3600.0,
                    |_| {},
                )?;
                println!(
                    "{}",
                    json!({
                        "run": run,
                        "prompt_tokens": prompt.len(),
                        "generated_tokens": generation.token_ids.len(),
                        "finish_reason": generation.finish_reason,
                        "prefill_seconds": generation.prefill_seconds,
                        "timing": timing,
                        "token_ids": generation.token_ids,
                        "answer": generation.answer,
                    })
                );
            }
        }
        _ => {
            bail!("usage: psionic-flint-metal-bench (generate|logits) --weights DIR --request FILE")
        }
    }
    Ok(())
}
