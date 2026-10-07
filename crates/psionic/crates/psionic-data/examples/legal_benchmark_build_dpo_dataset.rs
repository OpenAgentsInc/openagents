use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;

use psionic_data::{LegalDpoDatasetBuilderConfig, build_legal_benchmark_dpo_dataset};

fn main() -> Result<(), Box<dyn Error>> {
    let mut runs = None;
    let mut out = None;
    let mut manifest = None;
    let args = env::args().skip(1).collect::<Vec<_>>();
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
            other => {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument `{other}`"),
                )));
            }
        }
        index += 1;
    }
    let runs = runs.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_build_dpo_dataset --runs <runs-dir> --out <dataset.jsonl> [--manifest <manifest.json>]",
        )
    })?;
    let out = out.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: legal_benchmark_build_dpo_dataset --runs <runs-dir> --out <dataset.jsonl> [--manifest <manifest.json>]",
        )
    })?;
    let manifest = manifest.unwrap_or_else(|| {
        let mut path = out.clone();
        path.set_extension("manifest.json");
        path
    });
    let result = build_legal_benchmark_dpo_dataset(&LegalDpoDatasetBuilderConfig {
        runs_root: runs,
        out_jsonl: out,
        manifest_json: manifest,
        dataset_id: String::from("legal-dpo-v1"),
    })?;
    println!(
        "wrote legal DPO dataset: included={} excluded={} hash={}",
        result.manifest.included_count,
        result.manifest.excluded_count,
        result.manifest.dataset_hash
    );
    Ok(())
}
