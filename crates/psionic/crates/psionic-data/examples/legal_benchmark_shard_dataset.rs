use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_data::{LegalBenchmarkDatasetShardConfig, build_legal_benchmark_dataset_shards};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut dataset = None;
    let mut shards = None;
    let mut out = None;
    let mut dataset_id = String::from("legal-sft-v1");
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dataset" => {
                index += 1;
                dataset = args.get(index).map(PathBuf::from);
            }
            "--shards" => {
                index += 1;
                shards = args
                    .get(index)
                    .map(|value| value.parse::<u32>())
                    .transpose()?;
            }
            "--out" => {
                index += 1;
                out = args.get(index).map(PathBuf::from);
            }
            "--dataset-id" => {
                index += 1;
                dataset_id = args.get(index).cloned().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "--dataset-id requires a value")
                })?;
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
    let usage = "usage: legal_benchmark_shard_dataset --dataset <dataset.jsonl> --shards <n> --out <dir> [--dataset-id <id>]";
    let dataset = dataset.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;
    let shards = shards.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;
    let out = out.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, usage))?;

    let result = build_legal_benchmark_dataset_shards(&LegalBenchmarkDatasetShardConfig {
        dataset_jsonl: dataset,
        shard_count: shards,
        out_dir: out,
        dataset_id,
    })?;
    println!(
        "wrote legal benchmark shard manifest: path={} examples={} shards={} dataset_hash={} manifest_hash={}",
        result.manifest_path.display(),
        result.manifest.example_count,
        result.manifest.shard_count,
        result.manifest.dataset_global_hash,
        result.manifest.manifest_hash
    );
    Ok(())
}
