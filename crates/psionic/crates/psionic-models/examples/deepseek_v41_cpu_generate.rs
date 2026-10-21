//! Offline CPU text generation from the pinned V4.1-Flash checkpoint.
//! This scalar reference path is for comparison, not production throughput.

use psionic_models::{
    deepseek_v41::{DeepSeekV41Protocol, render_deepseek_v41_checkpoint_case},
    deepseek_v41_cpu::{
        engram::CompressedTokenMap,
        generation::GreedyOptions,
        quantization::Arithmetic,
        transformer::{CpuTransformer, TransformerConfig},
    },
    deepseek_v41_weights::{CheckpointOpenOptions, DeepSeekV41Checkpoint},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    time::Instant,
};

const HELP: &str = "Offline V4.1-Flash CPU runner
Usage:
  deepseek_v41_cpu_generate DIR --prompt TEXT [OPTIONS]
  deepseek_v41_cpu_generate DIR --prompt-tokens FILE [OPTIONS]
  deepseek_v41_cpu_generate DIR --check-only [--verify-sha256]
Options:
  --max-tokens N       Output token limit, including EOS (default: 32)
  --context N          Prompt plus output token capacity (default: 4096)
  --prefill-chunk N    Tokens per prefill call (default: 128)
  --oracle-tp8         Match the eight-rank oracle arithmetic profile
  --verify-sha256      Hash all 48 shards, reading about 510 GB
  --check-only         Check checkpoint metadata and pinned tokenizer; do not generate
  --help              Show this help without opening a checkpoint
TEXT becomes one user message in the pinned DS1 chat template.
FILE contains a JSON array of raw token IDs, with no template added.
The CPU reference arithmetic and original weights still need oracle comparison.
";

#[derive(Debug, PartialEq, Eq)]
struct Args {
    dir: PathBuf,
    prompt: Option<String>,
    prompt_tokens: Option<PathBuf>,
    max_tokens: usize,
    context: usize,
    prefill_chunk: usize,
    verify_sha256: bool,
    oracle_tp8: bool,
    check_only: bool,
}
#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Run(Args),
}

fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let args: Vec<_> = args.into_iter().collect();
    if args == ["--help"] || args == ["-h"] {
        return Ok(Command::Help);
    }
    let mut iter = args.into_iter();
    let mut dir = None;
    let mut result = Args {
        dir: PathBuf::new(),
        prompt: None,
        prompt_tokens: None,
        max_tokens: 32,
        context: 4096,
        prefill_chunk: 128,
        verify_sha256: false,
        oracle_tp8: false,
        check_only: false,
    };
    let mut seen = BTreeSet::new();
    while let Some(arg) = iter.next() {
        if arg.starts_with('-') && !seen.insert(arg.clone()) {
            return Err(format!("{arg} was given twice"));
        }
        let mut value = || iter.next().ok_or_else(|| format!("{arg} needs a value"));
        let number = |v: String| {
            v.parse::<usize>()
                .map_err(|_| format!("{arg} needs a nonnegative integer"))
        };
        match arg.as_str() {
            "--prompt" => result.prompt = Some(value()?),
            "--prompt-tokens" => result.prompt_tokens = Some(PathBuf::from(value()?)),
            "--max-tokens" => result.max_tokens = number(value()?)?,
            "--context" => result.context = number(value()?)?,
            "--prefill-chunk" => result.prefill_chunk = number(value()?)?,
            "--verify-sha256" => result.verify_sha256 = true,
            "--oracle-tp8" => result.oracle_tp8 = true,
            "--check-only" => result.check_only = true,
            other if !other.starts_with('-') && dir.is_none() => dir = Some(PathBuf::from(other)),
            other => return Err(format!("unknown or misplaced argument: {other}")),
        }
    }
    result.dir = dir.ok_or("Pass the checkpoint directory; use --help for usage")?;
    if result.context == 0 || result.context > 1_048_576 || result.prefill_chunk == 0 {
        return Err("Use --context 1..1048576 and a positive --prefill-chunk".into());
    }
    if result.check_only {
        if seen
            .iter()
            .any(|a| !["--check-only", "--verify-sha256"].contains(&a.as_str()))
        {
            return Err("--check-only accepts only DIR and --verify-sha256".into());
        }
    } else if result.prompt.is_some() == result.prompt_tokens.is_some() {
        return Err("Choose exactly one of --prompt or --prompt-tokens".into());
    }
    Ok(Command::Run(result))
}

fn run(args: Args) -> Result<Value, Box<dyn std::error::Error>> {
    let started = Instant::now();
    let checkpoint =
        DeepSeekV41Checkpoint::open(&args.dir, CheckpointOpenOptions::pinned_flash()?)?;
    let identity = if args.verify_sha256 {
        checkpoint.verify_shard_hashes(None)?
    } else {
        checkpoint.identity()
    };
    let protocol = DeepSeekV41Protocol::from_checkpoint_files(
        &fs::read(args.dir.join("tokenizer.json"))?,
        &fs::read(args.dir.join("tokenizer_config.json"))?,
    )?;
    let open_ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut report = json!({
        "schema":"psionic.deepseek_v41.cpu_generation.v1",
        "checkpoint":identity,
        "tokenizer":protocol.identity(),
        "all_shard_hashes_verified":args.verify_sha256,
        "arithmetic":if args.oracle_tp8 {"reference_bf16_tp8_cpu"}else{"reference_bf16_cpu"},
        "qualification":{"original_weight_numerics_compared":false,"device_parity_compared":false},
        "timing_ms":{"open_and_optional_hashing":open_ms},
    });
    if args.check_only {
        report["mode"] = json!("check_only");
        return Ok(report);
    }
    let started = Instant::now();
    let tokens: Vec<u32> = if let Some(path) = &args.prompt_tokens {
        serde_json::from_slice(&fs::read(path)?)?
    } else {
        let case = json!({"messages":[{"role":"user","content":args.prompt.as_deref().ok_or("prompt missing")?}],"thinking_mode":"chat"});
        protocol.encode_rendered(&render_deepseek_v41_checkpoint_case(&case.to_string())?)?
    };
    if tokens.is_empty() || tokens.len() > args.context || tokens.iter().any(|t| *t >= 129280) {
        return Err("The prompt must contain 1..context valid token IDs; increase --context or shorten the prompt".into());
    }
    let tokenizer = protocol.tokenizer();
    let map = Arc::new(CompressedTokenMap::build(&tokenizer, 99092)?);
    let mut model = CpuTransformer::new(
        &checkpoint,
        TransformerConfig::flash()?,
        map,
        args.context,
        if args.oracle_tp8 {
            Arithmetic::ReferenceBf16Tp8
        } else {
            Arithmetic::ReferenceBf16
        },
    )?;
    let prepare_ms = started.elapsed().as_secs_f64() * 1000.0;
    let started = Instant::now();
    let output = model.generate_greedy(
        &tokens,
        GreedyOptions {
            max_output_tokens: args.max_tokens,
            prefill_chunk_size: args.prefill_chunk,
        },
        || false,
    )?;
    let generation_ms = started.elapsed().as_secs_f64() * 1000.0;
    let text = tokenizer
        .decode(&output.generated_tokens, true)
        .map_err(|e| io::Error::other(e.to_string()))?;
    report["mode"] = json!("greedy");
    report["prompt_encoding"] = json!(if args.prompt_tokens.is_some() {
        "raw_token_ids"
    } else {
        "ds1_single_user_chat"
    });
    report["limits"] = json!({"context":args.context,"max_output_tokens":args.max_tokens,"prefill_chunk_size":args.prefill_chunk});
    report["generation"] = serde_json::to_value(output)?;
    report["text"] = json!(text);
    report["timing_ms"]["prepare_tokenizer_map_and_model"] = json!(prepare_ms);
    report["timing_ms"]["prefill_and_decode"] = json!(generation_ms);
    Ok(report)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let command = parse(env::args().skip(1)).map_err(io::Error::other)?;
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    match command {
        Command::Help => stdout.write_all(HELP.as_bytes())?,
        Command::Run(args) => {
            serde_json::to_writer_pretty(&mut stdout, &run(args)?)?;
            writeln!(stdout)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Result<Command, String> {
        parse(values.iter().map(|s| s.to_string()))
    }
    #[test]
    fn help_and_valid_modes_need_no_checkpoint() -> Result<(), String> {
        assert_eq!(args(&["--help"])?, Command::Help);
        let Command::Run(parsed) = args(&[
            "DIR",
            "--prompt",
            "hello",
            "--max-tokens",
            "0",
            "--context",
            "8",
            "--prefill-chunk",
            "2",
            "--verify-sha256",
        ])?
        else {
            return Err("run expected".into());
        };
        assert_eq!(parsed.max_tokens, 0);
        assert_eq!(parsed.context, 8);
        assert_eq!(parsed.prefill_chunk, 2);
        assert!(parsed.verify_sha256);
        assert!(args(&["DIR", "--prompt-tokens", "tokens.json", "--oracle-tp8"]).is_ok());
        assert!(args(&["DIR", "--check-only", "--verify-sha256"]).is_ok());
        Ok(())
    }
    #[test]
    fn invalid_arguments_are_refused_before_io() {
        for values in [
            vec![],
            vec!["DIR"],
            vec!["DIR", "--prompt"],
            vec!["DIR", "--prompt", "a", "--prompt-tokens", "b"],
            vec!["DIR", "--prompt", "a", "--temperature", "0"],
            vec!["DIR", "--prompt", "a", "--max-tokens", "-1"],
            vec![
                "DIR",
                "--prompt",
                "a",
                "--max-tokens",
                "999999999999999999999999999999999",
            ],
            vec!["DIR", "--prompt", "a", "--context", "0"],
            vec!["DIR", "--prompt", "a", "--context", "1048577"],
            vec!["DIR", "--prompt", "a", "--prefill-chunk", "0"],
            vec!["DIR", "--prompt", "a", "--prompt", "b"],
            vec!["DIR", "--check-only", "--prompt", "a"],
            vec!["DIR", "--check-only", "--max-tokens", "0"],
            vec!["DIR", "OTHER", "--prompt", "a"],
            vec!["--help", "--unsupported"],
        ] {
            assert!(args(&values).is_err(), "{values:?}");
        }
    }
}
