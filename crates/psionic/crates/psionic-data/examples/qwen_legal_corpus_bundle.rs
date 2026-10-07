use std::{env, error::Error, io, path::PathBuf};

use psionic_data::{build_qwen_legal_corpus_bundle, QwenLegalCorpusBundleConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut runs = None;
    let mut out = None;
    let mut corpus_id = None;
    let mut sft_shards = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--runs" => {
                index += 1;
                runs = args.get(index).map(PathBuf::from);
            }
            "--out" => {
                index += 1;
                out = args.get(index).map(PathBuf::from);
            }
            "--corpus-id" => {
                index += 1;
                corpus_id = args.get(index).cloned();
            }
            "--sft-shards" => {
                index += 1;
                sft_shards = args
                    .get(index)
                    .map(|value| value.parse::<u32>())
                    .transpose()?;
            }
            other => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument `{other}`"),
                )));
            }
        }
        index += 1;
    }
    let runs = runs.ok_or_else(usage)?;
    let out = out.ok_or_else(usage)?;
    let mut config = QwenLegalCorpusBundleConfig::new(runs, out);
    if let Some(corpus_id) = corpus_id {
        config.corpus_id = corpus_id;
    }
    if let Some(shards) = sft_shards {
        config.sft_shard_count = shards;
    }
    let result = build_qwen_legal_corpus_bundle(&config)?;
    println!(
        "wrote Qwen legal corpus bundle: corpus={} sft_train={} sft_holdout={} dpo_train={} dpo_holdout={} grpo_seeds={} grpo_holdout={} trace_records={} pylon_shards={} manifest={} receipt={} hash={}",
        result.manifest.corpus_id,
        result.manifest.sft_train_count,
        result.manifest.sft_holdout_count,
        result.manifest.dpo_train_count,
        result.manifest.dpo_holdout_count,
        result.manifest.grpo_seed_count,
        result.manifest.grpo_holdout_count,
        result.manifest.trace_store_count,
        result.manifest.pylon_shard_refs.len(),
        result.manifest_path.display(),
        result.receipt_path.display(),
        result.manifest.manifest_hash
    );
    Ok(())
}

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: qwen_legal_corpus_bundle --runs <runs-dir> --out <bundle-dir> [--corpus-id <id>] [--sft-shards <count>]",
    )
}
