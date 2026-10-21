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
    /// Teacher-forced continuation and its reference log-probabilities.
    #[serde(default)]
    continuation: Vec<u32>,
    #[serde(default)]
    reference_logprobs: Vec<f64>,
    #[serde(default)]
    stepped: bool,
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
                    model.prefill(&mut session, &[*token], 0)?;
                } else {
                    model.decode(&mut session, *token, 0)?;
                }
                model.logits(&mut logits);
                println!(
                    "{}",
                    json!({"position": position, "token": token, "logits": logits})
                );
            }
        }
        "prefill-check" => {
            // Chunked prefill against token-by-token decode on the same prompt.
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
            let (mut chunked, mut stepped) = (Vec::new(), Vec::new());
            let mut session = model.session(context)?;
            model.prefill(&mut session, &prompt, 0)?;
            model.logits(&mut chunked);
            let mut session = model.session(context)?;
            for token in &prompt {
                model.prefill(&mut session, &[*token], 0)?;
            }
            model.logits(&mut stepped);
            let top = |v: &[f32]| {
                (0..v.len())
                    .max_by(|a, b| v[*a].total_cmp(&v[*b]).then(b.cmp(a)))
                    .unwrap_or(0)
            };
            let softmax = |v: &[f32]| {
                let m = v.iter().copied().fold(f32::MIN, f32::max) as f64;
                let e: Vec<f64> = v.iter().map(|x| (*x as f64 - m).exp()).collect();
                let z: f64 = e.iter().sum();
                e.into_iter().map(|x| x / z).collect::<Vec<_>>()
            };
            let (a, b) = (softmax(&chunked), softmax(&stepped));
            let tv: f64 = a.iter().zip(&b).map(|(x, y)| (x - y).abs()).sum::<f64>() / 2.0;
            let max_abs = chunked
                .iter()
                .zip(&stepped)
                .map(|(x, y)| (x - y).abs())
                .fold(0f32, f32::max);
            println!(
                "{}",
                json!({"prompt_tokens": prompt.len(), "top_chunked": top(&chunked), "top_stepped": top(&stepped),
                       "total_variation": tv, "max_abs_logit_difference": max_abs})
            );
        }
        "teacher" => {
            // Feed a reference continuation; report this path's view of each token.
            let text = render(&request.messages)?;
            let prompt = tokenizer
                .encode(text, false)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
                .get_ids()
                .to_vec();
            let mut session = model.session(context)?;
            if request.stepped {
                for token in &prompt {
                    model.prefill(&mut session, &[*token], 0)?;
                }
            } else {
                model.prefill(&mut session, &prompt, 0)?;
            }
            let mut logits = Vec::new();
            let mut rows = Vec::new();
            for (i, token) in request.continuation.iter().enumerate() {
                model.logits(&mut logits);
                let top = (0..logits.len())
                    .max_by(|a, b| logits[*a].total_cmp(&logits[*b]).then(b.cmp(a)))
                    .unwrap_or(0);
                let m = logits.iter().copied().fold(f32::MIN, f32::max) as f64;
                let z: f64 = logits.iter().map(|x| (*x as f64 - m).exp()).sum();
                let logprob = logits[*token as usize] as f64 - m - z.ln();
                rows.push(
                    json!({"i": i, "top": top, "token": token, "logprob": logprob,
                    "reference": request.reference_logprobs.get(i)}),
                );
                model.decode(&mut session, *token, 0)?;
            }
            println!(
                "{}",
                json!({"prompt_tokens": prompt.len(), "stepped": request.stepped, "rows": rows})
            );
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
