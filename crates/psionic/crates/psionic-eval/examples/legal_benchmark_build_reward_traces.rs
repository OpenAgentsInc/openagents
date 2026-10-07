use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_eval::{build_legal_benchmark_reward_traces, LegalRewardTraceBuilderConfig};

fn main() -> Result<(), Box<dyn Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut runs = None;
    let mut out = None;
    let mut manifest = None;
    let mut dataset_id = None;
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
            "--manifest" => {
                index += 1;
                manifest = args.get(index).map(PathBuf::from);
            }
            "--dataset-id" => {
                index += 1;
                dataset_id = args.get(index).cloned();
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
    let mut config = LegalRewardTraceBuilderConfig::new(runs, out);
    if let Some(path) = manifest {
        config.manifest_json = path;
    }
    if let Some(id) = dataset_id {
        config.dataset_id = id;
    }
    let result = build_legal_benchmark_reward_traces(&config)?;
    println!(
        "wrote legal reward traces: total={} included={} excluded={} hash={}",
        result.manifest.total_count,
        result.manifest.included_count,
        result.manifest.excluded_count,
        result.manifest.dataset_hash
    );
    Ok(())
}

fn usage() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "usage: legal_benchmark_build_reward_traces --runs <runs-dir> --out <dataset.jsonl> [--manifest <manifest.json>] [--dataset-id <id>]",
    )
}
